# Test: SHALL-19-14-03-006
# Obligation: "If a filename begins with a <period> ('.'), the <period> shall
#   be explicitly matched"
# (Duplicate of SHALL-19-14-03-002)
# Verifies: * and ? do not match leading dot in filenames.

mkdir -p "$TMPDIR/dotdup"
: > "$TMPDIR/dotdup/.hidden"
: > "$TMPDIR/dotdup/visible"
cd "$TMPDIR/dotdup"

found=no
for f in *; do
    case "$f" in .hidden) found=yes ;; esac
done
if [ "$found" = "yes" ]; then
    printf '%s\n' "FAIL: * matched .hidden" >&2
    exit 1
fi
rm -rf "$TMPDIR/dotdup"
exit 0
