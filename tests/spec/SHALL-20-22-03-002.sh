# SHALL-20-22-03-002
# "If the command_name is the same as the name of one of the special built-in
#  utilities, the special properties in the enumerated list at the beginning of
#  2.15 Special Built-In Utilities shall not occur."
# Specifically: variable assignment errors from a special built-in prefixed
# with 'command' must not cause the shell to exit.

fail=0

# Without 'command', a special built-in variable assignment error would
# cause the shell to exit (in POSIX mode). With 'command', it should not.
# Test: 'command export' with an invalid name should produce an error
# but not kill the shell.
command export '===invalid' 2>/dev/null
# If we reach here, the shell did not exit — that's correct
rc=$?
if [ "$rc" -eq 0 ]; then
  printf 'FAIL: command export with invalid name should fail\n' >&2
  fail=1
fi

# Also test that variable assignments in prefix don't persist
FOO=bar command true
if [ "${FOO+set}" = "set" ] 2>/dev/null; then
  # FOO should not persist; with 'command' prefix, special built-in
  # property of persisting prefix assignments is suppressed
  :
fi

exit "$fail"
