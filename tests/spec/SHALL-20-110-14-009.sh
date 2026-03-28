# reviewed: GPT-5.4
# SHALL-20-110-14-009
# "The following exit values shall be returned:: A specified command_file
#  could not be found by a non-interactive shell."
# Verifies docs/posix/issue8/sh-utility.html#tag_20_110_14 for missing
# command_file exit status, across both slash and bare-name operand forms
# described in docs/posix/issue8/sh-utility.html#tag_20_110_05.

SH="${MEIKSH:-sh}"
TMP_BASE=${TMPDIR:-/tmp}
WORK="$TMP_BASE/shall_20_110_14_009_$$"
PATH_DIR="$WORK/path"
SLASH_PATH="$WORK/nonexistent_with_slash.sh"
BARE_NAME="nonexistent_bare_name_$$.sh"

rm -rf "$WORK"
mkdir -p "$PATH_DIR" || exit 1

"$SH" "$SLASH_PATH" >/dev/null 2>&1
rc=$?
if [ "$rc" -ne 127 ]; then
    printf '%s\n' "FAIL: missing slash command_file exited $rc, expected 127" >&2
    exit 1
fi

(
    cd "$WORK" || exit 1
    PATH="$PATH_DIR" "$SH" "$BARE_NAME" >/dev/null 2>&1
) 
rc=$?
if [ "$rc" -ne 127 ]; then
    printf '%s\n' "FAIL: missing bare command_file exited $rc, expected 127" >&2
    exit 1
fi

rm -rf "$WORK"
exit 0
