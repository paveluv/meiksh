# SHALL-20-53-03-014
# "Otherwise, the shell variable specified by name shall be set to the
#  <question-mark> character, the shell variable OPTARG shall be unset, and a
#  diagnostic message shall be written to standard error."
# Verify verbose-mode missing-argument handling: name='?', OPTARG unset, stderr diagnostic, exit 0.

# optstring "b:" without leading colon = verbose mode
# Pass -b with no argument
OPTIND=1
unset OPTARG
_err=$(eval 'getopts "b:" opt -b' 2>&1)
_rc=$?

if [ "$opt" != "?" ]; then
  printf '%s\n' "FAIL: name should be '?' but got '$opt'" >&2
  exit 1
fi

# OPTARG must be unset
if [ "${OPTARG+set}" = "set" ]; then
  printf '%s\n' "FAIL: OPTARG should be unset but is '$OPTARG'" >&2
  exit 1
fi

# diagnostic must be written to stderr (captured in _err)
if [ -z "$_err" ]; then
  printf '%s\n' "FAIL: no diagnostic written to stderr" >&2
  exit 1
fi

# exit status must be 0 (application error, not getopts error)
if [ "$_rc" -ne 0 ]; then
  printf '%s\n' "FAIL: exit status should be 0 but got $_rc" >&2
  exit 1
fi

exit 0
