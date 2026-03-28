# SHALL-08-03-010
# "This variable shall represent the terminal type for which output is to
#  be prepared."
# Verify TERM is propagated to child processes.

TERM=xterm
export TERM
_val=$(sh -c 'printf "%s" "$TERM"')
if [ "$_val" != "xterm" ]; then
  printf '%s\n' "FAIL: TERM not propagated to child" >&2
  exit 1
fi

exit 0
