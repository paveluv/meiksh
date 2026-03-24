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
export RUSTFLAGS="${RUSTFLAGS-} -Cinstrument-coverage"

cd "$repo_root"
cargo test

"$llvm_profdata" merge -sparse "$profile_dir"/*.profraw -o "$profdata"

objects=""
for path in \
    "$repo_root"/target/debug/deps/meiksh-* \
    "$repo_root"/target/debug/deps/spec_basic-* \
    "$repo_root"/target/debug/deps/differential_portable-* \
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
    --ignore-filename-regex='(/rustc/|/\.cargo/registry/)' \
    --instr-profile "$profdata" \
    $objects)

printf '%s\n' "$coverage_summary"

"$llvm_cov" export \
    --format=text \
    --summary-only \
    --ignore-filename-regex='(/rustc/|/\.cargo/registry/)' \
    --instr-profile "$profdata" \
    $objects > "$profile_dir/summary.json"

"$llvm_cov" show \
    --format=text \
    --show-instantiations=false \
    --ignore-filename-regex='(/rustc/|/\.cargo/registry/)' \
    --instr-profile "$profdata" \
    $objects > "$profile_dir/files.txt"
