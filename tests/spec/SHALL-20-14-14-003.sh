# SHALL-20-14-14-003
# "The following exit values shall be returned:: The current working directory
#  was successfully changed and the value of the PWD environment variable was
#  set correctly."
# Verify cd exits 0 on successful directory change.

"${SHELL}" -c 'cd / && exit 0'
rc=$?
if [ "$rc" -ne 0 ]; then
  printf '%s\n' "FAIL: cd / should exit 0, got $rc" >&2
  exit 1
fi

"${SHELL}" -c 'cd /tmp && exit 0'
rc=$?
if [ "$rc" -ne 0 ]; then
  printf '%s\n' "FAIL: cd /tmp should exit 0, got $rc" >&2
  exit 1
fi

exit 0
