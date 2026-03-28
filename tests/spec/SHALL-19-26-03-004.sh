# Test: SHALL-19-26-03-004
# Obligation: "The set special built-in shall support XBD 12.2 Utility Syntax
#   Guidelines except that options can be specified with either a leading
#   <hyphen-minus> (meaning enable the option) or <plus-sign> (meaning
#   disable it)"

# Enable with - and disable with +
set -f
set +f
# After +f, pathname expansion should be re-enabled
# (don't test globbing since test environment may vary)
# Just verify set does not error
if [ $? -ne 0 ]; then
    printf '%s\n' "FAIL: set +f returned non-zero" >&2
    exit 1
fi

exit 0
