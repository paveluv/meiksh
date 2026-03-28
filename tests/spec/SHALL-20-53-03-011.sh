# SHALL-20-53-03-011
# "If an option-argument is missing:: If the first character of optstring is a
#  <colon>, the shell variable specified by name shall be set to the <colon>
#  character and the shell variable OPTARG shall be set to the option character
#  found."
# Verify missing option-argument in silent mode: name=':', OPTARG=option char.

fail=0

OPTIND=1
getopts ":a:" opt -a 2>/dev/null
ret=$?
if [ "$opt" != ":" ]; then
  echo "FAIL: silent missing-arg opt is '$opt', expected ':'" >&2
  fail=1
fi
if [ "$OPTARG" != "a" ]; then
  echo "FAIL: silent missing-arg OPTARG is '$OPTARG', expected 'a'" >&2
  fail=1
fi
if [ "$ret" -ne 0 ]; then
  echo "FAIL: silent missing-arg exit status is $ret, expected 0" >&2
  fail=1
fi

exit "$fail"
