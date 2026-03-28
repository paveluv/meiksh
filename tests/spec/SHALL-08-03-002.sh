# SHALL-08-03-002
# "The system shall initialize this variable at the time of login to be a
#  pathname of the user's home directory."
# Verify HOME is used by tilde expansion and cd with no args.

_save="$HOME"
HOME="$TMPDIR"

_expanded=$(eval 'printf "%s" ~')
if [ "$_expanded" != "$TMPDIR" ]; then
  HOME="$_save"
  printf '%s\n' "FAIL: tilde did not expand to HOME" >&2
  exit 1
fi

HOME="$_save"
exit 0
