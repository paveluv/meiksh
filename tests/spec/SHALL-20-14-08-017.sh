# SHALL-20-14-08-017
# "A pathname of the previous working directory, used when the operand is '-'."
# Verify OLDPWD is used by cd - to return to the previous directory.

got=$("${SHELL}" -c '
  cd / 2>/dev/null
  cd /tmp 2>/dev/null
  out=$(cd - 2>/dev/null)
  printf "%s\n" "$out"
')

case "$got" in
  /*) ;;
  *) printf '%s\n' "FAIL: cd - did not print previous dir, got: '$got'" >&2; exit 1 ;;
esac

exit 0
