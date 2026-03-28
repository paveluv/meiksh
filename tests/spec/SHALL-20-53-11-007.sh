# SHALL-20-53-11-007
# "If an option requiring an option-argument is found, but an option-argument
#  is not found, this error shall be identified and the invalid option
#  character shall be identified in the message."
# Verify getopts error for missing option-argument identifies the option.

err=$("${SHELL}" -c '
  OPTIND=1
  getopts "a:b" opt -a
' 2>&1 >/dev/null)

case "$err" in
  *a*) ;;
  *) printf '%s\n' "FAIL: getopts should identify option 'a' with missing arg, got: $err" >&2; exit 1 ;;
esac

exit 0
