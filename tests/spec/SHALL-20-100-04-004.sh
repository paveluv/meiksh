# SHALL-20-100-04-004
# "If delim consists of one single-byte character, that byte shall be used
#  as the logical line delimiter. If delim is the null string, the logical
#  line delimiter shall be the null byte."
# Verifies: -d with a single-byte char uses that as delimiter;
#   -d '' uses null byte as delimiter.

# -d with single byte delimiter
printf 'hello:world\n' | {
  read -d : var
  if [ "$var" != "hello" ]; then
    printf '%s\n' "FAIL: -d ':' var='$var' expected 'hello'" >&2; exit 1
  fi
}
[ $? -ne 0 ] && exit 1

# -d '' uses null byte delimiter
printf 'abc\0def\n' | {
  read -d '' var
  if [ "$var" != "abc" ]; then
    printf '%s\n' "FAIL: -d '' var='$var' expected 'abc'" >&2; exit 1
  fi
}
[ $? -ne 0 ] && exit 1

exit 0
