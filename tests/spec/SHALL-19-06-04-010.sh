# Test: SHALL-19-06-04-010
# Obligation: "If the expression is invalid, or the contents of a shell
#   variable used in the expression are not recognized by the shell, the
#   expansion fails and the shell shall write a diagnostic message to standard
#   error indicating the failure."
# Verifies: invalid arithmetic produces error on stderr and nonzero exit.

# Non-numeric variable should cause failure
x=abc
msg=$(eval ': $((x + 1))' 2>&1) && {
    printf '%s\n' "FAIL: non-numeric var in arith did not fail" >&2
    exit 1
}
if [ -z "$msg" ]; then
    printf '%s\n' "FAIL: no diagnostic on stderr for non-numeric var" >&2
    exit 1
fi

# Invalid expression syntax
msg2=$(eval ': $((1 +))' 2>&1) && {
    printf '%s\n' "FAIL: invalid arith expression did not fail" >&2
    exit 1
}
if [ -z "$msg2" ]; then
    printf '%s\n' "FAIL: no diagnostic on stderr for invalid arith" >&2
    exit 1
fi

exit 0
