# SHALL-20-53-11-006
# "If an option is found that was not specified in optstring, this error is
#  identified and the invalid option character shall be identified in the
#  message."
# Verify getopts error message contains the invalid option character.

err=$("${SHELL}" -c '
  OPTIND=1
  getopts "ab" opt -z
' 2>&1 >/dev/null)

case "$err" in
  *z*) ;;
  *) printf '%s\n' "FAIL: getopts should identify invalid option 'z', got: $err" >&2; exit 1 ;;
esac

exit 0
