# SHALL-19-05-02-006
# "$- (Hyphen.) Expands to the current option flags ... The -i option shall be
#  included in \"$-\" if the shell is interactive, regardless of whether it was
#  specified on invocation."
# Verify $- contains active option flags.

fail=0

# $- should be non-empty (at minimum the shell has some default flags)
[ -n "$-" ] || { printf '%s\n' "FAIL: \$- is empty" >&2; fail=1; }

# After 'set -x', $- should contain 'x'
set -x
case "$-" in
  *x*) ;;
  *) printf '%s\n' "FAIL: \$- missing 'x' after set -x: '$-'" >&2; fail=1 ;;
esac
set +x

# After 'set +x', $- should NOT contain 'x'
case "$-" in
  *x*) printf '%s\n' "FAIL: \$- still contains 'x' after set +x: '$-'" >&2; fail=1 ;;
esac

exit "$fail"
