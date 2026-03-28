# SHALL-18-01-01-02-004
# "Independent processes shall be capable of executing independently without
#  either process terminating."
# Verify pipeline components run concurrently without either terminating the other.

result=$("${SHELL}" -c 'printf "%s\n" "abc" | cat')
if [ "$result" != "abc" ]; then
  printf '%s\n' "FAIL: pipeline failed, got '$result'" >&2
  exit 1
fi

exit 0
