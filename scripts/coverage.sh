#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
profile_dir="$repo_root/target/coverage"
profdata="$profile_dir/meiksh.profdata"
host_triple=$(rustc -vV | awk '/host:/ {print $2}')
tool_dir="$(rustc --print sysroot)/lib/rustlib/$host_triple/bin"
llvm_cov="$tool_dir/llvm-cov"
llvm_profdata="$tool_dir/llvm-profdata"
ignore_regex='(/rustc/|/\.cargo/registry/|/tests/)'

rm -rf "$profile_dir"
mkdir -p "$profile_dir"

export CARGO_INCREMENTAL=0
export RUSTFLAGS="${RUSTFLAGS-} -Cinstrument-coverage --cfg coverage"

cd "$repo_root"
python3 - "$repo_root" <<'PY'
import pathlib
import sys

repo_root = pathlib.Path(sys.argv[1])
for pattern in ("target/debug/deps/meiksh-*", "target/debug/deps/spec_basic-*", "target/debug/meiksh"):
    for path in repo_root.glob(pattern):
        try:
            path.unlink()
        except FileNotFoundError:
            pass
PY

# ── Step 1: build everything (real binary + tests) and run tests ──
export LLVM_PROFILE_FILE="$profile_dir/meiksh-%p-%m.profraw"
cargo build --lib --bin meiksh
cargo test --lib --test integration_basic

"$llvm_profdata" merge -sparse "$profile_dir"/meiksh-*.profraw -o "$profdata"

objects=""
for path in \
    "$repo_root"/target/debug/deps/meiksh-* \
    "$repo_root"/target/debug/deps/integration_basic-* \
    "$repo_root"/target/debug/meiksh
do
    if [ -f "$path" ] && [ -x "$path" ]; then
        case "$path" in
            *.d|*.rlib|*.rmeta) ;;
            *) objects="$objects --object $path" ;;
        esac
    fi
done

coverage_summary=$("$llvm_cov" report \
    --ignore-filename-regex="$ignore_regex" \
    --instr-profile "$profdata" \
    $objects)

printf '%s\n' "$coverage_summary"

"$llvm_cov" export \
    --format=text \
    --summary-only \
    --ignore-filename-regex="$ignore_regex" \
    --instr-profile "$profdata" \
    $objects > "$profile_dir/summary.json"

"$llvm_cov" export \
    --format=lcov \
    --ignore-filename-regex="$ignore_regex" \
    --instr-profile "$profdata" \
    $objects > "$profile_dir/lcov.info"

"$llvm_cov" show \
    --format=text \
    --show-instantiations=false \
    --ignore-filename-regex="$ignore_regex" \
    --instr-profile "$profdata" \
    $objects > "$profile_dir/files.txt"

# ── Step 2: compute production-only coverage ──
# The production binary contains only non-#[cfg(test)] code.  Asking llvm-cov
# to report on just that object (with the test profdata for format compat)
# gives us the exact set of production lines — no source parsing needed.
"$llvm_cov" export \
    --format=lcov \
    --ignore-filename-regex="$ignore_regex" \
    --instr-profile "$profdata" \
    --object "$repo_root/target/debug/meiksh" \
    > "$profile_dir/prod-lcov.info"
python3 - "$repo_root" "$profile_dir/prod-lcov.info" "$profile_dir/lcov.info" \
    > "$profile_dir/production-line-summary.txt" <<'PY'
import json
import pathlib
import sys

repo_root = pathlib.Path(sys.argv[1])
prod_lcov = pathlib.Path(sys.argv[2])
test_lcov = pathlib.Path(sys.argv[3])


def parse_lcov_lines(lcov_path: pathlib.Path) -> dict[pathlib.Path, dict[int, int]]:
    result: dict[pathlib.Path, dict[int, int]] = {}
    current = None
    for raw_line in lcov_path.read_text().splitlines():
        if raw_line.startswith("SF:"):
            p = pathlib.Path(raw_line[3:])
            if str(p).startswith(str(repo_root / "src")) and p.exists():
                current = p
                result.setdefault(current, {})
            else:
                current = None
            continue
        if not raw_line.startswith("DA:") or current is None:
            continue
        line_no, count = raw_line[3:].split(",")[:2]
        line_no = int(line_no)
        count = int(count)
        result[current][line_no] = max(result[current].get(line_no, 0), count)
    return result


prod_lines = parse_lcov_lines(prod_lcov)
test_lines = parse_lcov_lines(test_lcov)

found = 0
hit = 0
for path, lines in prod_lines.items():
    test = test_lines.get(path, {})
    for line_no in lines:
        found += 1
        if test.get(line_no, 0) > 0:
            hit += 1

coverage = 100.0 if found == 0 else (hit / found * 100.0)
print(
    f"Production-only line coverage: "
    f"{coverage:.2f}% ({hit}/{found})"
)
print(json.dumps({"hit": hit, "found": found, "coverage": coverage}))
PY

sed -n '1p' "$profile_dir/production-line-summary.txt"
