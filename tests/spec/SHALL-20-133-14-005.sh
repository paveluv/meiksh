# SHALL-20-133-14-005
# "The following exit values shall be returned:: One of the alias-name operands
#  specified did not represent a valid alias definition, or an error occurred."
# Verify unalias returns >0 for an unknown alias name.

unalias _nonexistent_alias_xyz_ 2>/dev/null
_rc=$?
if [ "$_rc" -eq 0 ]; then
  printf '%s\n' "FAIL: unalias returned 0 for invalid alias name" >&2
  exit 1
fi

exit 0
