# SHALL-20-53-03-013
# "If the first character of optstring is a <colon>, the shell variable specified
#  by name shall be set to the <colon> character and the shell variable OPTARG
#  shall be set to the option character found."
# Verify silent mode missing-arg handling (duplicate of 011).

fail=0

OPTIND=1
getopts ":b:" opt -b 2>/dev/null
if [ "$opt" != ":" ]; then
  echo "FAIL: opt is '$opt', expected ':'" >&2
  fail=1
fi
if [ "$OPTARG" != "b" ]; then
  echo "FAIL: OPTARG is '$OPTARG', expected 'b'" >&2
  fail=1
fi

exit "$fail"
