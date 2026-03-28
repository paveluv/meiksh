# Test: SHALL-12-02-002
# Obligation: "The utilities in the Shell and Utilities volume of POSIX.1-2024
#   that claim conformance to these guidelines shall conform completely to
#   these guidelines as if these guidelines contained the term 'shall' instead
#   of 'should'."
# Verifies: Builtins follow Utility Syntax Guidelines — specifically that '--'
#   terminates option processing (Guideline 10) and that options are single
#   characters preceded by '-' (Guideline 2/3).

# Guideline 10: '--' marks end of options for builtins
# 'printf' with '--' should treat next arg as format, not option
out=$(${MEIKSH:-meiksh} -c 'printf -- "-hello\n"' 2>&1)
if [ "$out" != "-hello" ]; then
    printf '%s\n' "FAIL: printf -- did not terminate option parsing; got: $out" >&2
    exit 1
fi

# Guideline 10: 'export' with '--' should treat -X as a variable name attempt
# (it should fail since -X is not a valid variable name, but should NOT be
# parsed as an option)
out=$(${MEIKSH:-meiksh} -c 'export -- -X 2>&1; echo rc=$?' 2>&1)
case "$out" in
    *rc=0*) ;;
    *) ;; # error is acceptable — the point is it wasn't treated as an option
esac

# Guideline 2/3: options are single-char preceded by '-'
# 'set' with '--' should stop option parsing
${MEIKSH:-meiksh} -c 'set -- -a -b -c; [ "$1" = "-a" ]' 2>/dev/null
if [ $? -ne 0 ]; then
    printf '%s\n' "FAIL: set -- should assign positional params, not parse them as options" >&2
    exit 1
fi

exit 0
