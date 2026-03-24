#!/bin/sh

set -eu

DOCS_DIR=$(CDPATH= cd -- "$(dirname "$0")" && pwd)
POSIX_DIR="$DOCS_DIR/posix"
MANIFEST="$DOCS_DIR/posix-manifest.txt"

if [ ! -f "$MANIFEST" ]; then
    printf '%s\n' "missing manifest: $MANIFEST" >&2
    exit 1
fi

echo "Creating directories..."
mkdir -p "$POSIX_DIR"

echo "Fetching POSIX mirror from manifest..."
while IFS='|' read -r relative_path url group; do
    case "$relative_path" in
        ''|'#'*)
            continue
            ;;
    esac

    target="$POSIX_DIR/$relative_path"
    target_dir=$(dirname "$target")
    mkdir -p "$target_dir"

    printf '  [%s] %s\n' "$group" "$relative_path"
    curl -LfsS "$url" -o "$target"
done < "$MANIFEST"

echo "Done! All manifest documents have been fetched to $POSIX_DIR"
