# SHALL-20-110-14-009
# "The following exit values shall be returned:: A specified command_file
#  could not be found by a non-interactive shell."
# Verifies: sh exits 127 when command_file does not exist.

f="$TMPDIR/nonexistent_$$_does_not_exist.sh"
rm -f "$f"
"$MEIKSH" "$f" >/dev/null 2>&1
rc=$?
if [ "$rc" -ne 127 ]; then
    printf '%s\n' "FAIL: missing command_file exited $rc, expected 127" >&2
    exit 1
fi

exit 0
