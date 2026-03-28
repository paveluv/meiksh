# reviewed: GPT-5.4
# Test: SHALL-20-110-09-003
# Obligation: "If the shell is interactive: SIGQUIT and SIGTERM signals
#   shall be ignored."
# Verifies: An interactive shell ignores SIGQUIT and SIGTERM.

SH="${MEIKSH:-${SHELL:-sh}}"

# Start an interactive shell, send it SIGTERM and SIGQUIT, verify it survives
result=$(printf 'kill -TERM $$\nkill -QUIT $$\nprintf "%%s\\n" "alive"\n' | "$SH" -i 2>/dev/null)
case "$result" in
    *alive*)
        ;;
    *)
        printf '%s\n' "FAIL: interactive shell did not survive SIGTERM, got '$result'" >&2
        exit 1
        ;;
esac

exit 0
