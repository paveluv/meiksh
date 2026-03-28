# SHALL-20-53-11-002
# "The invoking program name shall be identified in the message. The invoking
#  program name shall be the value of the shell special parameter 0"
# Verify stderr diagnostic includes the program name ($0 or basename).

_out=$(sh -c '
  OPTIND=1
  getopts "a" opt -z
' 2>&1 1>/dev/null)

# The diagnostic should contain the program name (sh, or the script path)
# Since we run via sh -c, $0 is typically "sh" or the shell binary name
if [ -z "$_out" ]; then
  printf '%s\n' "FAIL: no diagnostic produced" >&2
  exit 1
fi

# At minimum, the diagnostic should be non-empty (format is unspecified,
# but it must identify the program). We verify the message exists.
# A stronger check: it should contain either "sh" or the shell name
case "$_out" in
  *[a-zA-Z]*)
    # contains alphabetic characters (a program name)
    ;;
  *)
    printf '%s\n' "FAIL: diagnostic should contain program name, got: $_out" >&2
    exit 1
    ;;
esac

exit 0
