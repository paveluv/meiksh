# Test: SHALL-19-23-03-005
# Obligation: "The shell shall format the output, including the proper use of
#   quoting, so that it is suitable for reinput to the shell as commands that
#   achieve the same exporting results"

# export -p output should be valid shell that re-creates exports
export EXPORT_REINPUT="hello world"
output=$(export -p)
case "$output" in
    *'export EXPORT_REINPUT='*)
        # Output contains the expected format
        ;;
    *)
        printf '%s\n' "FAIL: export -p format not suitable for reinput" >&2
        exit 1
        ;;
esac

exit 0
