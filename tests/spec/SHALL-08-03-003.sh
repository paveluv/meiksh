# SHALL-08-03-003
# "This variable shall represent a decimal integer >0 used to indicate the
#  user's preferred number of lines on a page or the vertical screen or
#  window size in lines."
# Verify LINES is propagated to children.

LINES=50
export LINES
_val=$(sh -c 'printf "%s" "$LINES"')
if [ "$_val" != "50" ]; then
  printf '%s\n' "FAIL: LINES not propagated to child" >&2
  exit 1
fi

exit 0
