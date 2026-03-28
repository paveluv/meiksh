# SHALL-20-53-03-005
# "In all other cases, the value of OPTIND is unspecified, but shall encode the
#  information needed for the next invocation of getopts to resume parsing options
#  after the option just parsed."
# Verify grouped options are parsed correctly across multiple getopts calls.

fail=0

OPTIND=1
result=""
while getopts "abc" opt -abc; do
  result="${result}${opt}"
done

if [ "$result" != "abc" ]; then
  echo "FAIL: grouped options result '$result' != expected 'abc'" >&2
  fail=1
fi

exit "$fail"
