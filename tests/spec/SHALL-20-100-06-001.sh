# SHALL-20-100-06-001
# "If the -d delim option is not specified, or if it is specified and delim
#  is not the null string, the standard input shall contain zero or more
#  bytes (which need not form valid characters) and shall not contain any
#  null bytes."
# Verifies: read handles arbitrary non-null byte input without -d ''.

# Read arbitrary bytes (high bytes, not valid UTF-8 necessarily)
printf '\x80\x81\x82\n' | {
  read -r var
  if [ -z "$var" ]; then
    printf '%s\n' "FAIL: empty var from arbitrary bytes" >&2; exit 1
  fi
}
[ $? -ne 0 ] && exit 1

exit 0
