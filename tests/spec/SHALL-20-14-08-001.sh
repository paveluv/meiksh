# SHALL-20-14-08-001
# "The following environment variables shall affect the execution of cd:"
# Verify cd is affected by the key environment variables (CDPATH, HOME, etc.).

# HOME affects cd with no arguments
got=$("${SHELL}" -c '
  HOME=/tmp
  export HOME
  cd
  pwd -P
')
expected=$(cd /tmp && pwd -P)
if [ "$got" != "$expected" ]; then
  printf '%s\n' "FAIL: cd without args did not use HOME, got: $got" >&2
  exit 1
fi

exit 0
