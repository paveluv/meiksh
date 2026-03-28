# SHALL-08-03-008
# "This variable shall represent a pathname of the user's preferred command
#  language interpreter."
# Verify SHELL is propagated to child processes.

SHELL=/bin/sh
export SHELL
_val=$(sh -c 'printf "%s" "$SHELL"')
if [ "$_val" != "/bin/sh" ]; then
  printf '%s\n' "FAIL: SHELL not propagated to child" >&2
  exit 1
fi

exit 0
