# Test: SHALL-19-28-03-001
# Obligation: "The times utility shall write the accumulated user and system
#   times for the shell and for all of its child processes, in the following
#   POSIX locale format:"

# times should produce two lines of output in NmN.NNs format
output=$(times)
line1=$(printf '%s\n' "$output" | head -1)
line2=$(printf '%s\n' "$output" | sed -n '2p')

if [ -z "$line1" ] || [ -z "$line2" ]; then
    printf '%s\n' "FAIL: times did not produce two lines of output" >&2
    exit 1
fi

# Verify format: each line should contain "m" and "s" characters
case "$line1" in
    *m*s*m*s*) ;;
    *)
        printf '%s\n' "FAIL: times line 1 format incorrect: $line1" >&2
        exit 1
        ;;
esac

case "$line2" in
    *m*s*m*s*) ;;
    *)
        printf '%s\n' "FAIL: times line 2 format incorrect: $line2" >&2
        exit 1
        ;;
esac

exit 0
