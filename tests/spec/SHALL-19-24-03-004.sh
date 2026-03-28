# Test: SHALL-19-24-03-004
# Obligation: "The shell shall format the output, including the proper use of
#   quoting, so that it is suitable for reinput to the shell as commands that
#   achieve the same value and readonly attribute-setting results"

# readonly -p lists readonly variables
readonly RO_P_TEST=pval
output=$(readonly -p)
case "$output" in
    *RO_P_TEST*) ;;
    *)
        printf '%s\n' "FAIL: readonly -p did not list RO_P_TEST" >&2
        exit 1
        ;;
esac

exit 0
