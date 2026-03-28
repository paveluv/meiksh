# SHALL-20-100-04-001
# "The read utility shall conform to XBD 12.2 Utility Syntax Guidelines."
# Verifies: read accepts -- to end options, and grouped options.

# -- ends option processing
printf 'hello\n' | {
  read -- var
  if [ "$var" != "hello" ]; then
    printf '%s\n' "FAIL: read -- var: var='$var'" >&2; exit 1
  fi
}
[ $? -ne 0 ] && exit 1

# -r option works
printf 'a\\b\n' | {
  read -r var
  if [ "$var" != 'a\b' ]; then
    printf '%s\n' "FAIL: read -r: var='$var' expected 'a\\b'" >&2; exit 1
  fi
}
[ $? -ne 0 ] && exit 1

exit 0
