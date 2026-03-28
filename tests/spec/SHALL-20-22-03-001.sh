# SHALL-20-22-03-001
# "The command utility shall cause the shell to treat the arguments as a simple
#  command, suppressing the shell function lookup that is described in 2.9.1.4
#  Command Search and Execution, item 1c."

fail=0

# Define a function that would return 55
ls() { return 55; }

# Calling 'ls' should invoke the function
ls >/dev/null 2>&1
rc=$?
if [ "$rc" -ne 55 ]; then
  printf 'FAIL: function ls should return 55, got %d\n' "$rc" >&2
  fail=1
fi

# 'command ls' must skip the function and run the real ls utility
command ls / >/dev/null 2>&1
rc=$?
if [ "$rc" -eq 55 ]; then
  printf 'FAIL: command ls invoked function instead of utility\n' >&2
  fail=1
fi

unset -f ls

exit "$fail"
