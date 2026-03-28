# reviewed: GPT-5.4
# SHALL-20-110-14-007
# "The following exit values shall be returned:: A specified command_file
#  could not be executed due to an [ENOEXEC] error (see 2.9.1.4 Command
#  Search and Execution, item 2)."
# Verifies: sh exits 126 when command_file is rejected as not being a script.

f="$TMPDIR/enoexec_$$.bin"
# Create a file whose prefix contains NUL before newline.
printf '\000bad\n' > "$f"
SH="${MEIKSH:-${SHELL:-sh}}"

"$SH" "$f" >/dev/null 2>&1
rc=$?
if [ "$rc" -ne 126 ]; then
    printf '%s\n' "FAIL: ENOEXEC script exited $rc, expected 126" >&2
    rm -f "$f"
    exit 1
fi
rm -f "$f"

exit 0
