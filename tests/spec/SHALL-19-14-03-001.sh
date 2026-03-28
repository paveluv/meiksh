# Test: SHALL-19-14-03-001
# Obligation: "The <slash> character in a pathname shall be explicitly matched
#   by using one or more <slash> characters in the pattern; it shall neither be
#   matched by the <asterisk> or <question-mark> special characters nor by a
#   bracket expression."
# Verifies: glob * and ? do not match / in pathnames.

mkdir -p "$TMPDIR/globslash/sub"
: > "$TMPDIR/globslash/sub/file"
: > "$TMPDIR/globslash/top"

cd "$TMPDIR/globslash"

# * should not match across /
matched=no
for f in *; do
    case "$f" in
        sub/file) matched=yes ;;
    esac
done
if [ "$matched" = "yes" ]; then
    printf '%s\n' "FAIL: * matched across / boundary" >&2
    exit 1
fi

# ? should not match /
case "a/b" in a?b) printf '%s\n' "FAIL: ? matched / in case pattern" >&2; exit 1 ;; *) ;; esac

rm -rf "$TMPDIR/globslash"
exit 0
