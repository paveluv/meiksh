#!/bin/sh

set -eu

repo_root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
manifest="$repo_root/docs/posix-manifest.txt"
posix_dir="$repo_root/docs/posix"

if [ ! -f "$manifest" ]; then
    printf '%s\n' "missing manifest: $manifest" >&2
    exit 1
fi

missing=0
checked=0

while IFS='|' read -r relative_path _url group; do
    case "$relative_path" in
        ''|'#'*)
            continue
            ;;
    esac

    checked=$((checked + 1))
    target="$posix_dir/$relative_path"
    if [ ! -s "$target" ]; then
        printf 'missing [%s] %s\n' "$group" "$target" >&2
        missing=$((missing + 1))
    fi
done < "$manifest"

if [ "$missing" -ne 0 ]; then
    printf 'POSIX mirror check failed: %d missing of %d expected pages\n' "$missing" "$checked" >&2
    exit 1
fi

printf 'POSIX mirror check passed: %d expected pages present\n' "$checked"
