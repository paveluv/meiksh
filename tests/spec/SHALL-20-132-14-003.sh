# SHALL-20-132-14-003
# "The following exit values shall be returned:: The file mode creation mask
#  was successfully changed, or no mask operand was supplied."
# Verify umask exits 0 on success (both set and query).

# Query mode (no operand)
"${SHELL}" -c 'umask'
rc=$?
if [ "$rc" -ne 0 ]; then
  printf '%s\n' "FAIL: umask query exited $rc, expected 0" >&2
  exit 1
fi

# Set mode
"${SHELL}" -c 'umask 022'
rc=$?
if [ "$rc" -ne 0 ]; then
  printf '%s\n' "FAIL: umask 022 exited $rc, expected 0" >&2
  exit 1
fi

# Symbolic mode
"${SHELL}" -c 'umask u=rwx,go=rx'
rc=$?
if [ "$rc" -ne 0 ]; then
  printf '%s\n' "FAIL: umask symbolic exited $rc, expected 0" >&2
  exit 1
fi

exit 0
