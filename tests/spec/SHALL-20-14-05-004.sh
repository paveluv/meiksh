# SHALL-20-14-05-004
# "If directory is an empty string, cd shall write a diagnostic message to
#  standard error and exit with non-zero status. If directory consists of a
#  single '-', the cd utility shall behave as if directory contained the value
#  of the OLDPWD environment variable"
# Verify cd "" fails and cd - uses OLDPWD.

# cd "" must fail
"${SHELL}" -c 'cd ""' 2>/dev/null
rc=$?
if [ "$rc" -eq 0 ]; then
  printf '%s\n' "FAIL: cd '' should exit non-zero" >&2
  exit 1
fi

# cd "" must write to stderr
err=$("${SHELL}" -c 'cd ""' 2>&1 >/dev/null)
if [ -z "$err" ]; then
  printf '%s\n' "FAIL: cd '' should write diagnostic to stderr" >&2
  exit 1
fi

# cd - must use OLDPWD and print new directory to stdout
got=$("${SHELL}" -c '
  cd / 2>/dev/null
  cd /tmp 2>/dev/null
  cd - 2>/dev/null
')
case "$got" in
  /*) ;;
  *) printf '%s\n' "FAIL: cd - should print new directory, got: '$got'" >&2; exit 1 ;;
esac

exit 0
