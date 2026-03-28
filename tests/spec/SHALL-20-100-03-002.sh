# SHALL-20-100-03-002
# "If the -r option is not specified, <backslash> shall act as an escape character."

# Test backslash-newline continuation (without -r)
result=$("$MEIKSH" -c '
  read var <<EOF
hello\
world
EOF
  printf "%s\n" "$var"
')
if [ "$result" != "helloworld" ]; then
  printf '%s\n' "FAIL: backslash-newline not handled as continuation, got: $result" >&2
  exit 1
fi

# Test backslash removal
result=$("$MEIKSH" -c '
  read var <<EOF
hel\\lo
EOF
  printf "%s\n" "$var"
')
if [ "$result" != "hel\\lo" ]; then
  # After backslash processing: \\ -> \, so result should be "hel\lo"
  if [ "$result" != 'hel\lo' ]; then
    printf '%s\n' "FAIL: backslash escape not handled, got: $result" >&2
    exit 1
  fi
fi
exit 0
