# SHALL-20-53-03-003
# "When getopts successfully parses an option that takes an option-argument [...]
#  the value of OPTIND shall be the integer index of the next element of the
#  parameter list."
# Verify OPTIND points past consumed option-argument.

fail=0

OPTIND=1
getopts "a:" opt -a value extra
if [ "$OPTIND" -ne 3 ]; then
  echo "FAIL: OPTIND after -a value is $OPTIND, expected 3" >&2
  fail=1
fi
if [ "$OPTARG" != "value" ]; then
  echo "FAIL: OPTARG is '$OPTARG', expected 'value'" >&2
  fail=1
fi

# Concatenated form: -avalue
OPTIND=1
getopts "a:" opt -avalue extra
if [ "$OPTIND" -ne 2 ]; then
  echo "FAIL: OPTIND after -avalue is $OPTIND, expected 2" >&2
  fail=1
fi
if [ "$OPTARG" != "value" ]; then
  echo "FAIL: OPTARG is '$OPTARG', expected 'value'" >&2
  fail=1
fi

exit "$fail"
