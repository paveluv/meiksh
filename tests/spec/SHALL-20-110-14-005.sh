# reviewed: GPT-5.4
# SHALL-20-110-14-005
# "The following exit values shall be returned:: A non-interactive shell
#  detected an error other than command_file not found, command_file not
#  executable, or an unrecoverable read error while reading commands ...
#  including but not limited to syntax, redirection, or variable assignment
#  errors."
# Verifies: sh exits 1-125 for syntax errors, redirection errors, etc.

SH="${MEIKSH:-${SHELL:-sh}}"

# Syntax error
f="$TMPDIR/syntax_$$.sh"
printf 'if then\n' > "$f"
"$SH" "$f" >/dev/null 2>&1
rc=$?
if [ "$rc" -lt 1 ] || [ "$rc" -gt 125 ]; then
    printf '%s\n' "FAIL: syntax error exited $rc, expected 1-125" >&2
    rm -f "$f"
    exit 1
fi
rm -f "$f"

# Redirection error (writing to unwritable path)
f="$TMPDIR/redir_$$.sh"
printf 'echo x > /dev/null/impossible\n' > "$f"
"$SH" "$f" >/dev/null 2>&1
rc=$?
if [ "$rc" -lt 1 ] || [ "$rc" -gt 125 ]; then
    printf '%s\n' "FAIL: redirection error exited $rc, expected 1-125" >&2
    rm -f "$f"
    exit 1
fi
rm -f "$f"

exit 0
