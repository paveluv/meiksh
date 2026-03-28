# reviewed: GPT-5.4
# Test: SHALL-20-110-11-001
# Obligation: "Except as otherwise stated (by the descriptions of any invoked
#   utilities or in interactive mode), standard error shall be used only for
#   diagnostic messages."
# Verifies: A syntax error in non-interactive mode produces a diagnostic
#   message on stderr and nothing on stdout.

SH="${MEIKSH:-${SHELL:-sh}}"

stdout=$("$SH" -c 'if then fi' 2>/dev/null) || true
if [ -n "$stdout" ]; then
    printf '%s\n' "FAIL: syntax error wrote to stdout: '$stdout'" >&2
    exit 1
fi

stderr=$("$SH" -c 'if then fi' 2>&1 >/dev/null) || true
if [ -z "$stderr" ]; then
    printf '%s\n' "FAIL: syntax error produced no diagnostic on stderr" >&2
    exit 1
fi

exit 0
