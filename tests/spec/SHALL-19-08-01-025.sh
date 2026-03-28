# Test: SHALL-19-08-01-025
# Obligation: "If an unrecoverable read error occurs when reading commands,
#   ... the shell shall execute no further commands ... other than any
#   specified in a previously defined EXIT trap action."
# Verifies: unrecoverable read error in dot-sourced file is treated as
#   special built-in error (shell exits non-interactively).

f="$TMPDIR/dot_test_$$"
printf '%s\n' 'echo sourced' > "$f"
chmod 000 "$f"
result=$("$SHELL" -c ". '$f'; echo ALIVE" 2>/dev/null)
chmod 644 "$f" 2>/dev/null
rm -f "$f"
case "$result" in
    *ALIVE*)
        printf '%s\n' "FAIL: shell should exit after dot-file read error" >&2
        exit 1
        ;;
esac

exit 0
