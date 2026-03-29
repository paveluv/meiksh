#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
profile_dir="$repo_root/target/coverage"
profdata="$profile_dir/meiksh.profdata"
host_triple=$(rustc -vV | awk '/host:/ {print $2}')
tool_dir="$(rustc --print sysroot)/lib/rustlib/$host_triple/bin"
llvm_cov="$tool_dir/llvm-cov"
llvm_profdata="$tool_dir/llvm-profdata"

rm -rf "$profile_dir"
mkdir -p "$profile_dir"

export CARGO_INCREMENTAL=0
export LLVM_PROFILE_FILE="$profile_dir/meiksh-%p-%m.profraw"
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
cargo test --lib --test integration_basic

"$llvm_profdata" merge -sparse "$profile_dir"/*.profraw -o "$profdata"

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
    --ignore-filename-regex='(/rustc/|/\.cargo/registry/|/tests/)' \
    --instr-profile "$profdata" \
    $objects)

printf '%s\n' "$coverage_summary"

"$llvm_cov" export \
    --format=text \
    --summary-only \
    --ignore-filename-regex='(/rustc/|/\.cargo/registry/|/tests/)' \
    --instr-profile "$profdata" \
    $objects > "$profile_dir/summary.json"

"$llvm_cov" export \
    --format=lcov \
    --ignore-filename-regex='(/rustc/|/\.cargo/registry/|/tests/)' \
    --instr-profile "$profdata" \
    $objects > "$profile_dir/lcov.info"

"$llvm_cov" show \
    --format=text \
    --show-instantiations=false \
    --ignore-filename-regex='(/rustc/|/\.cargo/registry/|/tests/)' \
    --instr-profile "$profdata" \
    $objects > "$profile_dir/files.txt"

python3 - "$repo_root" "$profile_dir/lcov.info" > "$profile_dir/production-line-summary.txt" <<'PY'
import json
import pathlib
import sys

repo_root = pathlib.Path(sys.argv[1])
lcov_path = pathlib.Path(sys.argv[2])


def inline_test_ranges(path: pathlib.Path) -> list[tuple[int, int]]:
    lines = path.read_text().splitlines()
    ranges = []
    i = 0
    while i < len(lines):
        if lines[i].strip() != "#[cfg(test)]":
            i += 1
            continue
        j = i + 1
        while j < len(lines) and "mod " not in lines[j]:
            j += 1
        if j >= len(lines):
            i += 1
            continue
        ranges.append((i + 1, len(lines)))
        break
    return ranges


def excluded_lines(path: pathlib.Path) -> set[int]:
    excl = set()
    for i, line in enumerate(path.read_text().splitlines(), 1):
        if "LCOV_EXCL_LINE" in line:
            excl.add(i)
    return excl


current = None
per_file = {}
excl_cache: dict[pathlib.Path, set[int]] = {}
line_counts: dict[tuple, int] = {}

for raw_line in lcov_path.read_text().splitlines():
    if raw_line.startswith("SF:"):
        current = pathlib.Path(raw_line[3:])
        if str(current).startswith(str(repo_root / "src")) and current.exists():
            per_file.setdefault(current, inline_test_ranges(current))
            excl_cache.setdefault(current, excluded_lines(current))
        else:
            current = None
        continue
    if not raw_line.startswith("DA:") or current is None:
        continue
    line_no, count = raw_line[3:].split(",")[:2]
    line_no = int(line_no)
    count = int(count)
    if any(start <= line_no <= end for start, end in per_file[current]):
        continue
    if line_no in excl_cache.get(current, set()):
        continue
    key = (current, line_no)
    line_counts[key] = max(line_counts.get(key, 0), count)

totals = {"found": len(line_counts), "hit": sum(1 for c in line_counts.values() if c > 0)}
coverage = 100.0 if totals["found"] == 0 else (totals["hit"] / totals["found"] * 100.0)
print(
    f"Production-only line coverage (excluding inline #[cfg(test)] modules): "
    f"{coverage:.2f}% ({totals['hit']}/{totals['found']})"
)
print(json.dumps({"hit": totals["hit"], "found": totals["found"], "coverage": coverage}))
PY

sed -n '1p' "$profile_dir/production-line-summary.txt"
