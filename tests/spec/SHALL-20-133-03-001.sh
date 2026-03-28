# SHALL-20-133-03-001
# "The unalias utility shall remove the definition for each alias name
#  specified. The aliases shall be removed from the current shell execution
#  environment."
# Verify unalias removes a defined alias.

alias _test_alias_xz='printf hello'
unalias _test_alias_xz
# After unalias, the alias should not be listed
_found=$(alias _test_alias_xz 2>/dev/null)
if [ -n "$_found" ]; then
  printf '%s\n' "FAIL: alias still defined after unalias" >&2
  exit 1
fi

exit 0
