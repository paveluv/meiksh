# SHALL-18-01-01-02-001
# "The following functionality of the fork() function defined in the System
#  Interfaces volume of POSIX.1-2024 shall be available on all systems
#  conforming to this volume of POSIX.1-2024"
# Verify the shell can fork child processes for external commands.

result=$("${MEIKSH:-meiksh}" -c 'printf "%s\n" hello')
if [ "$result" != "hello" ]; then
  printf '%s\n' "FAIL: fork/exec for subshell command failed" >&2
  exit 1
fi

exit 0
