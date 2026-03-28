# SHALL-20-53-05-003
# "A string containing the option characters recognized by the utility invoking
#  getopts. If a character is followed by a <colon>, the option shall be expected
#  to have an argument... getopts shall interpret the characters following an
#  option character requiring arguments as an argument whether or not this is
#  done... the value in OPTARG shall only be the characters of the option-argument."
# Verify optstring semantics: colon means argument required, concatenated form works.

# Test 1: option with argument as separate param
OPTIND=1
getopts "b:" opt -b value
if [ "$opt" != "b" ] || [ "$OPTARG" != "value" ]; then
  printf '%s\n' "FAIL(1): -b value: opt='$opt' OPTARG='$OPTARG'" >&2
  exit 1
fi

# Test 2: option with concatenated argument
OPTIND=1
getopts "b:" opt -bvalue
if [ "$opt" != "b" ] || [ "$OPTARG" != "value" ]; then
  printf '%s\n' "FAIL(2): -bvalue: opt='$opt' OPTARG='$OPTARG'" >&2
  exit 1
fi

# Test 3: OPTARG contains only the argument characters
OPTIND=1
getopts "b:" opt -bfoo
if [ "$OPTARG" != "foo" ]; then
  printf '%s\n' "FAIL(3): OPTARG should be 'foo', got '$OPTARG'" >&2
  exit 1
fi

# Test 4: option without colon takes no argument
OPTIND=1
getopts "a" opt -a
if [ "$opt" != "a" ]; then
  printf '%s\n' "FAIL(4): opt should be 'a', got '$opt'" >&2
  exit 1
fi

exit 0
