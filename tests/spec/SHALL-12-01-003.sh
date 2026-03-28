# Test: SHALL-12-01-003
# Obligation: "If an error is generated, the utility's diagnostic message shall
#   indicate that the value is out of the supported range, not that it is
#   syntactically incorrect."
# Verifies: When a builtin receives a numeric value out of supported range,
#   the error message says "out of range" (or similar), not "syntax error".

# Use 'exit' with a value beyond the normal 0-255 range to test numeric parsing.
# The shell should accept numeric syntax but may complain about range.
# Use 'kill' with a very large signal number as an out-of-range numeric test.
out=$(${MEIKSH:-meiksh} -c 'kill -s 99999999 $$ 2>&1' 2>&1) || true
case "$out" in
    *[Ss]yntax*|*[Ii]nvalid\ number*|*[Nn]ot\ a\ number*)
        printf '%s\n' "FAIL: error for out-of-range number incorrectly says syntax/invalid-number: $out" >&2
        exit 1
        ;;
esac

# Verify that an actually non-numeric value IS treated as a syntax/parse error
out2=$(${MEIKSH:-meiksh} -c 'kill -s notanumber $$ 2>&1' 2>&1) || true
if [ -z "$out2" ]; then
    printf '%s\n' "FAIL: kill -s notanumber should produce an error" >&2
    exit 1
fi

exit 0
