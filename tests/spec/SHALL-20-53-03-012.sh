# SHALL-20-53-03-012
# "If an option-argument is missing:: Otherwise, the shell variable specified by
#  name shall be set to the <question-mark> character, the shell variable OPTARG
#  shall be unset, and a diagnostic message shall be written to standard error.
#  [...] the exit status shall be zero."
# Verify missing option-argument in verbose mode: name='?', OPTARG unset, stderr.

fail=0

OPTIND=1
errout=$(getopts "a:" opt -a 2>&1)
ret=$?
if [ "$opt" != "?" ]; then
  echo "FAIL: verbose missing-arg opt is '$opt', expected '?'" >&2
  fail=1
fi
if [ "${OPTARG+set}" = "set" ]; then
  echo "FAIL: verbose missing-arg OPTARG is set ('$OPTARG'), expected unset" >&2
  fail=1
fi
if [ "$ret" -ne 0 ]; then
  echo "FAIL: verbose missing-arg exit status is $ret, expected 0" >&2
  fail=1
fi

exit "$fail"
