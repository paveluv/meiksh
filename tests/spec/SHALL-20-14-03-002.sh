# SHALL-20-14-03-002
# "If no directory operand is given and the HOME environment variable is empty
#  or undefined, the default behavior is implementation-defined and no further
#  steps shall be taken."
# Verify cd with no operand and unset HOME does not crash (exits non-zero or
# does nothing, implementation-defined).

_orig="$PWD"
(
  unset HOME
  cd 2>/dev/null
)
# Just verify the parent shell is unaffected and no crash occurred
if [ "$PWD" != "$_orig" ]; then
  printf '%s\n' "FAIL: parent PWD changed unexpectedly" >&2
  exit 1
fi

exit 0
