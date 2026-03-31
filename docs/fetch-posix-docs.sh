#!/bin/sh

set -eu

DOCS_DIR=$(CDPATH= cd -- "$(dirname "$0")" && pwd)
POSIX_DIR="$DOCS_DIR/posix"
MANIFEST="$DOCS_DIR/posix-manifest.txt"

if [ ! -f "$MANIFEST" ]; then
    printf '%s\n' "missing manifest: $MANIFEST" >&2
    exit 1
fi

archive_name=
archive_url=
archive_root=

while IFS='|' read -r manifest_archive_name manifest_archive_url manifest_archive_root; do
    case "$manifest_archive_name" in
        ''|'#'*)
            continue
            ;;
    esac

    archive_name=$manifest_archive_name
    archive_url=$manifest_archive_url
    archive_root=$manifest_archive_root
    break
done < "$MANIFEST"

if [ -z "$archive_name" ] || [ -z "$archive_url" ] || [ -z "$archive_root" ]; then
    printf '%s\n' "manifest must contain: archive_name|archive_url|archive_root" >&2
    exit 1
fi

tmpdir=$(mktemp -d)
cleanup() {
    rm -rf "$tmpdir"
}
trap cleanup EXIT INT HUP TERM

archive_path="$tmpdir/$archive_name"

echo "Downloading POSIX archive..."
printf '  %s\n' "$archive_url"
curl -LfsS "$archive_url" -o "$archive_path"

echo "Replacing docs/posix contents..."
rm -rf "$POSIX_DIR"
mkdir -p "$POSIX_DIR"

echo "Unpacking archive..."
tar -xzf "$archive_path" -C "$POSIX_DIR"

if [ ! -d "$POSIX_DIR/$archive_root" ]; then
    printf 'expected archive root missing after unpack: %s\n' "$POSIX_DIR/$archive_root" >&2
    exit 1
fi

printf 'Done! Archive unpacked to %s\n' "$POSIX_DIR"
