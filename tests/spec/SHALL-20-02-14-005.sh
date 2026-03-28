# SHALL-20-02-14-005
# "One of the name operands specified did not have an alias definition, or an
#  error occurred."

"$MEIKSH" -c 'alias nonexistent_alias_xyz 2>/dev/null; exit $?'
if [ $? -eq 0 ]; then
  printf '%s\n' "FAIL: alias returned 0 for undefined alias name" >&2
  exit 1
fi
exit 0
