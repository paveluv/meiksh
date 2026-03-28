# SHALL-20-53-11-004
# "If an option requiring an option-argument is found, but an option-argument is
#  not found, this error shall be identified and the invalid option character
#  shall be identified in the message."
# Verify missing-argument diagnostic includes the option character.

OPTIND=1
_err=$(getopts "b:" opt -b 2>&1 1>/dev/null)

case "$_err" in
  *b*)
    ;;
  *)
    printf '%s\n' "FAIL: diagnostic should contain option char 'b', got: $_err" >&2
    exit 1
    ;;
esac

exit 0
