# SHALL-20-53-03-008
# "In all other cases, the value of OPTIND is unspecified, but shall encode the
#  information needed for the next invocation of getopts to resume parsing options
#  after the option just parsed."
# Verify sequential getopts calls resume correctly with grouped and separate opts.

fail=0

OPTIND=1
result=""
while getopts "ab:c" opt -a -bc val -c; do
  result="${result}${opt}"
  case "$opt" in
    b) result="${result}=${OPTARG}" ;;
  esac
  result="${result},"
done

expected="a,b=c,c,"
# -bc: b takes c as its option-argument (since b: requires argument)
if [ "$result" != "$expected" ]; then
  echo "FAIL: resume parsing result '$result' != expected '$expected'" >&2
  fail=1
fi

exit "$fail"
