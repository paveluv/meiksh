# SHALL-20-100-06-002
# "If the -d delim option is specified and delim is the null string, the
#  standard input shall contain zero or more bytes (which need not form
#  valid characters)."
# Verifies: with -d '', null bytes in input are accepted as delimiters.

printf 'first\0second\n' | {
  read -d '' var
  if [ "$var" != "first" ]; then
    printf '%s\n' "FAIL: -d '' with null input: var='$var' expected 'first'" >&2
    exit 1
  fi
}
[ $? -ne 0 ] && exit 1

exit 0
