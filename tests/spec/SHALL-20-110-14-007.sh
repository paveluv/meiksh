# SHALL-20-110-14-007
# "The following exit values shall be returned:: A specified command_file
#  could not be executed due to an [ENOEXEC] error (see 2.9.1.4 Command
#  Search and Execution, item 2)."
# Verifies: sh exits 126 when command_file triggers ENOEXEC.

f="$TMPDIR/enoexec_$$.bin"
# Create a file with execute permission but invalid binary content
printf '\x01\x02\x03\x04' > "$f"
chmod +x "$f"
"$MEIKSH" "$f" >/dev/null 2>&1
rc=$?
if [ "$rc" -ne 126 ]; then
    printf '%s\n' "FAIL: ENOEXEC script exited $rc, expected 126" >&2
    rm -f "$f"
    exit 1
fi
rm -f "$f"

exit 0
