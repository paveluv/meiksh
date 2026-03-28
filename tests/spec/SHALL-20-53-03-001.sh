# SHALL-20-53-03-001
# "The getopts utility shall retrieve options and option-arguments from a list of
#  parameters. It shall support the Utility Syntax Guidelines 3 to 10, inclusive,
#  described in XBD 12.2 Utility Syntax Guidelines."
# Verify getopts parses options from a parameter list.

fail=0

test_getopts() {
  result=""
  OPTIND=1
  while getopts "ab:c" opt -a -b val -c; do
    result="${result}${opt}"
    case "$opt" in
      b) result="${result}=${OPTARG}" ;;
    esac
    result="${result},"
  done
  expected="a,b=val,c,"
  if [ "$result" != "$expected" ]; then
    echo "FAIL: getopts result '$result' != expected '$expected'" >&2
    fail=1
  fi
}

test_getopts
exit "$fail"
