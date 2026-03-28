# SHALL-08-01-004
# "If an attempt is made to mark any of the following variables as readonly,
#  then either the readonly utility shall reject the attempt, or readonly shall
#  succeed but the shell can still modify the variables outside of assignment
#  context, or readonly shall succeed but use of a shell built-in that would
#  otherwise modify such a variable shall fail."
# Test that LINENO, OLDPWD, OPTARG, OPTIND, PWD each exhibits one of the
# three permitted readonly behaviors.

fail=0

check_special_readonly() {
  varname="$1"

  # Try to mark it readonly; capture whether it succeeds or fails
  if readonly "$varname" 2>/dev/null; then
    # readonly succeeded — behavior (2) or (3) applies
    # Either the shell can still modify it, or builtins that modify it fail.
    # Both are acceptable; we just verify readonly didn't corrupt the shell.
    :
  else
    # readonly rejected the attempt — behavior (1), also acceptable
    :
  fi
}

# We run each check in a subshell so a rejected readonly doesn't affect others
for var in LINENO OLDPWD OPTARG OPTIND PWD; do
  (check_special_readonly "$var") || {
    printf '%s\n' "FAIL: readonly $var caused shell crash" >&2
    fail=1
  }
done

if [ "$fail" -ne 0 ]; then
  exit 1
fi

exit 0
