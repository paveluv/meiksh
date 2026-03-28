# SHALL-20-53-11-003
# "If an option is found that was not specified in optstring, this error is
#  identified and the invalid option character shall be identified in the message."
# Verify unknown-option diagnostic includes the invalid option character.

OPTIND=1
_err=$(getopts "a" opt -z 2>&1 1>/dev/null)

case "$_err" in
  *z*)
    ;;
  *)
    printf '%s\n' "FAIL: diagnostic should contain invalid char 'z', got: $_err" >&2
    exit 1
    ;;
esac

exit 0
