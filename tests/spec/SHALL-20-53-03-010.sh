# SHALL-20-53-03-010
# "If an option character not contained in the optstring operand is found where an
#  option character is expected, the shell variable specified by name shall be set
#  to the <question-mark> ('?') character. In this case, if the first character in
#  optstring is a <colon> (':'), the shell variable OPTARG shall be set to the
#  option character found, but no output shall be written to standard error;
#  otherwise, the shell variable OPTARG shall be unset and a diagnostic message
#  shall be written to standard error."
# Verify unknown option handling in both verbose and silent mode.

fail=0

# Verbose mode (no leading colon): name=?, OPTARG unset, stderr diagnostic
OPTIND=1
errout=$(getopts "ab" opt -z 2>&1)
if [ "$opt" != "?" ]; then
  echo "FAIL: verbose mode opt is '$opt', expected '?'" >&2
  fail=1
fi
if [ "${OPTARG+set}" = "set" ]; then
  echo "FAIL: verbose mode OPTARG is set ('$OPTARG'), expected unset" >&2
  fail=1
fi

# Silent mode (leading colon): name=?, OPTARG=z, no stderr
OPTIND=1
errout=$(getopts ":ab" opt -z 2>&1)
if [ "$opt" != "?" ]; then
  echo "FAIL: silent mode opt is '$opt', expected '?'" >&2
  fail=1
fi
if [ "$OPTARG" != "z" ]; then
  echo "FAIL: silent mode OPTARG is '$OPTARG', expected 'z'" >&2
  fail=1
fi

exit "$fail"
