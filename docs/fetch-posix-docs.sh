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

echo "Building html_to_md converter..."
REPO_DIR=$(CDPATH= cd -- "$DOCS_DIR/.." && pwd)
cargo build --quiet --bin html_to_md --manifest-path "$REPO_DIR/Cargo.toml"
converter="$REPO_DIR/target/debug/html_to_md"

HTML_ROOT="$POSIX_DIR/$archive_root"
MD_ROOT="$POSIX_DIR/md"
rm -rf "$MD_ROOT"

echo "Converting HTML to Markdown..."
converted=0
for dir in basedefs utilities functions xrat frontmatter help idx; do
    srcdir="$HTML_ROOT/$dir"
    [ -d "$srcdir" ] || continue
    destdir="$MD_ROOT/$dir"
    mkdir -p "$destdir"
    for html in "$srcdir"/*.html; do
        [ -f "$html" ] || continue
        base=$(basename "$html" .html)
        "$converter" "$html" "$destdir/$base.md"
        converted=$((converted + 1))
    done
done
for html in "$HTML_ROOT"/*.html; do
    [ -f "$html" ] || continue
    base=$(basename "$html" .html)
    "$converter" "$html" "$MD_ROOT/$base.md"
    converted=$((converted + 1))
done

printf 'Done! %d HTML files converted to Markdown in %s\n' "$converted" "$MD_ROOT"
