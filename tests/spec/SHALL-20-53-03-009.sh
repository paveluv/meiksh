# SHALL-20-53-03-009
# "When the option requires an option-argument, the getopts utility shall place it
#  in the shell variable OPTARG. If no option was found, or if the option that was
#  found does not have an option-argument, OPTARG shall be unset."
# Verify OPTARG is set for options with arguments and unset otherwise.

fail=0

# Option with argument: OPTARG should be set
OPTIND=1
getopts "a:" opt -a myarg
if [ "$OPTARG" != "myarg" ]; then
  echo "FAIL: OPTARG is '$OPTARG', expected 'myarg'" >&2
  fail=1
fi

# Option without argument: OPTARG should be unset
OPTIND=1
getopts "b" opt -b
if [ "${OPTARG+set}" = "set" ]; then
  echo "FAIL: OPTARG is set ('$OPTARG') after option without arg, expected unset" >&2
  fail=1
fi

# End of options: OPTARG should be unset
OPTIND=1
getopts "c" opt noopt 2>/dev/null || true
if [ "${OPTARG+set}" = "set" ]; then
  echo "FAIL: OPTARG is set ('$OPTARG') at end-of-options, expected unset" >&2
  fail=1
fi

exit "$fail"
