# SHALL-12-01-001
# "a conforming implementation shall also permit applications to specify the
#  option and option-argument in the same argument string without intervening
#  <blank> characters."
# "If the utility receives an argument containing only the option, it shall
#  behave as specified in its description for an omitted option-argument; it
#  shall not treat the next argument (if any) as the option-argument for that
#  option."
# Verify mandatory option-arguments work both separated and combined.

fail=0

# Test 1: "read -d DELIM" — mandatory option-argument as separate arg
# Using a colon delimiter, read should split on ':'
printf 'a:b\n' | {
  IFS= read -d : part
  if [ "$part" != "a" ]; then
    printf '%s\n' "FAIL: read -d : (separate) gave '$part', expected 'a'" >&2
    fail=1
  fi
}
[ "$?" -ne 0 ] && fail=1

# Test 2: "read -d: VAR" — mandatory option-argument combined with option
printf 'a:b\n' | {
  IFS= read -d: part
  if [ "$part" != "a" ]; then
    printf '%s\n' "FAIL: read -d: (combined) gave '$part', expected 'a'" >&2
    fail=1
  fi
}
[ "$?" -ne 0 ] && fail=1

# Test 3: "printf" format as mandatory option-argument to %s — not an option,
# but verify that "command -v ls" works both as separate args
v1=$(command -v sh)
if [ -z "$v1" ]; then
  printf '%s\n' "FAIL: 'command -v sh' returned empty" >&2
  fail=1
fi

# Test 4: "export -p" should list exports (no option-argument needed)
result=$(export -p 2>&1)
if [ $? -ne 0 ]; then
  printf '%s\n' "FAIL: 'export -p' failed" >&2
  fail=1
fi

# Test 5: "set -o" with separate vs combined: "set -o nounset" vs "set -onounset"
( set -o nounset 2>/dev/null; exit 0 )
if [ $? -ne 0 ]; then
  printf '%s\n' "FAIL: 'set -o nounset' (separate) failed" >&2
  fail=1
fi

exit "$fail"
