# Test: SHALL-19-23-03-004
# Obligation: "When -p is specified, export shall write to the standard output
#   the names and values of all exported variables"

# export -p lists exported variables
export EXPORT_P_TEST=pval
output=$(export -p)
case "$output" in
    *EXPORT_P_TEST*) ;;
    *)
        printf '%s\n' "FAIL: export -p did not list EXPORT_P_TEST" >&2
        exit 1
        ;;
esac

exit 0
