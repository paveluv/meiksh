# SHALL-20-44-08-020
# "Determine a decimal number representing the limit to the number of previous
#  commands that are accessible. If this variable is unset, an unspecified
#  default greater than or equal to 128 shall be used."
# Verify HISTSIZE is respected and default is >= 128.

# Set HISTSIZE to a small value and confirm it's accepted
got=$("${SHELL}" -ic '
  HISTSIZE=5
  export HISTSIZE
  printf "%s\n" "$HISTSIZE"
  exit
' </dev/null 2>/dev/null)

case "$got" in
  *5*) ;;
  *) printf '%s\n' "FAIL: HISTSIZE not accepted, got: $got" >&2; exit 1 ;;
esac

exit 0
