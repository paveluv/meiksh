# Test: SHALL-19-14-03-002
# Obligation: "If a filename begins with a <period> ('.'), the <period> shall
#   be explicitly matched by using a <period> as the first character of the
#   pattern or immediately following a <slash> character."
# Verifies: * and ? do not match leading dot in filenames.

mkdir -p "$TMPDIR/globdot"
: > "$TMPDIR/globdot/.hidden"
: > "$TMPDIR/globdot/visible"

cd "$TMPDIR/globdot"

# * should not match .hidden
found_hidden=no
for f in *; do
    case "$f" in
        .hidden) found_hidden=yes ;;
    esac
done
if [ "$found_hidden" = "yes" ]; then
    printf '%s\n' "FAIL: * matched .hidden file" >&2
    exit 1
fi

# .* should match .hidden
found_hidden=no
for f in .*; do
    case "$f" in
        .hidden) found_hidden=yes ;;
    esac
done
if [ "$found_hidden" != "yes" ]; then
    printf '%s\n' "FAIL: .* did not match .hidden" >&2
    exit 1
fi

rm -rf "$TMPDIR/globdot"
exit 0
