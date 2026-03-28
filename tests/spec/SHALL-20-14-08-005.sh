# SHALL-20-14-08-005
# "The following environment variables shall affect the execution of cd:: The
#  name of the directory, used when no directory operand is specified."
# Verify HOME is used as default directory when no operand is given.

got=$("${MEIKSH:-meiksh}" -c '
  HOME=/
  export HOME
  cd
  pwd -P
')
if [ "$got" != "/" ]; then
  printf '%s\n' "FAIL: cd with no args should go to HOME (/), got: $got" >&2
  exit 1
fi

exit 0
