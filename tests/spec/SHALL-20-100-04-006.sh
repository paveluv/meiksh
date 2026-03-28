# SHALL-20-100-04-006
# "Do not treat a <backslash> character in any special way. Consider each
#  <backslash> to be part of the input line."
# Verifies: -r treats backslash as literal; no line continuation.

# Backslash is literal with -r
printf 'hello\\nworld\n' | {
  read -r var
  if [ "$var" != 'hello\nworld' ]; then
    printf '%s\n' "FAIL: -r backslash literal: var='$var'" >&2; exit 1
  fi
}
[ $? -ne 0 ] && exit 1

# No line continuation with -r: backslash-newline is not continuation
printf 'line1\\\nline2\n' | {
  read -r var
  if [ "$var" != 'line1\' ]; then
    printf '%s\n' "FAIL: -r no continuation: var='$var' expected 'line1\\'" >&2; exit 1
  fi
}
[ $? -ne 0 ] && exit 1

exit 0
