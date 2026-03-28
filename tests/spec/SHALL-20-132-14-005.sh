# reviewed: GPT-5.4
# SHALL-20-132-14-005
# "The following exit values shall be returned:: An error occurred."
# Verify umask exits non-zero on invalid input.

"${SHELL}" -c 'umask 999' 2>/dev/null
rc=$?
if [ "$rc" -eq 0 ]; then
  printf '%s\n' "FAIL: umask 999 should exit non-zero, got 0" >&2
  exit 1
fi

"${SHELL}" -c 'umask abc' 2>/dev/null
rc=$?
if [ "$rc" -eq 0 ]; then
  printf '%s\n' "FAIL: umask abc should exit non-zero, got 0" >&2
  exit 1
fi

exit 0
