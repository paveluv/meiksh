# SHALL-08-03-001
# "This variable shall represent a decimal integer >0 used to indicate the
#  user's preferred width in column positions for the terminal screen or window"
# Verify COLUMNS is available to the shell and propagated.

COLUMNS=120
export COLUMNS
_val=$(sh -c 'printf "%s" "$COLUMNS"')
if [ "$_val" != "120" ]; then
  printf '%s\n' "FAIL: COLUMNS not propagated to child" >&2
  exit 1
fi

exit 0
