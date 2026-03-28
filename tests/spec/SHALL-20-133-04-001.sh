# SHALL-20-133-04-001
# "The unalias utility shall conform to XBD 12.2 Utility Syntax Guidelines."
# Verify unalias accepts -- to end option processing.

alias _test_xz_dash='printf hi'
unalias -- _test_xz_dash
_found=$(alias _test_xz_dash 2>/dev/null)
if [ -n "$_found" ]; then
  printf '%s\n' "FAIL: 'unalias --' did not work" >&2
  exit 1
fi

exit 0
