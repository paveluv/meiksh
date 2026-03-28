# SHALL-09-02-003
# "The implementation shall support any regular expression that does not
#  exceed 256 bytes in length."
# Verify the shell supports patterns up to at least 256 bytes.
# We construct a 256-char case pattern.

_pat="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
_str="$_pat"
case "$_str" in
  "$_pat") ;;
  *) printf '%s\n' "FAIL: 256-byte pattern not supported" >&2; exit 1 ;;
esac

exit 0
