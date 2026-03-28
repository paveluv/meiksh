# SHALL-20-100-03-006
# "The input to the algorithm shall be the logical line (minus terminating
#  delimiter) ... with any escaped byte and the preceding <backslash>
#  escape character treated as if they were the result of a quoted expansion,
#  and all other bytes treated as if they were the results of unquoted
#  expansions."
# Verifies: escaped IFS characters are not used as field separators.

IFS=' '
# backslash-space should not split
printf 'hello\\ world rest\n' | {
  read a b
  if [ "$a" != "hello world" ]; then
    printf '%s\n' "FAIL: escaped space should not split: a='$a' b='$b'" >&2
    exit 1
  fi
}
[ $? -ne 0 ] && exit 1

exit 0
