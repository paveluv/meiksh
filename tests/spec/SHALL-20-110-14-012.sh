# SHALL-20-110-14-012
# "Otherwise, the shell shall terminate in the same manner as for an exit
#  command with no operands, unless the last command the shell invoked was
#  executed without forking, in which case the wait status seen by the parent
#  process of the shell shall be the wait status of the last command the shell
#  invoked."
# Verify shell exit status equals $? of last command when reaching EOF.

# A script whose last command exits 42 should itself exit 42
_out=$(sh -c '(exit 42)'; printf '%s' "$?")
if [ "$_out" != "42" ]; then
  printf '%s\n' "FAIL: expected 42, got $_out" >&2
  exit 1
fi

# A script whose last command exits 0 should itself exit 0
sh -c 'true'
if [ "$?" != "0" ]; then
  printf '%s\n' "FAIL: expected 0 from 'true' script" >&2
  exit 1
fi

exit 0
