# Test: SHALL-19-28-03-002
# Obligation: "The four pairs of times shall correspond to the members of the
#   <sys/times.h> tms structure ... tms_utime, tms_stime, tms_cutime, and
#   tms_cstime, respectively."

# Verify times produces exactly 4 time values (2 lines, 2 pairs each)
output=$(times)
count=$(printf '%s\n' "$output" | wc -l)
if [ "$count" -lt 2 ]; then
    printf '%s\n' "FAIL: times produced fewer than 2 lines" >&2
    exit 1
fi

# Each line should have 2 time values (user and system)
line1=$(printf '%s\n' "$output" | head -1)
# Count occurrences of 'm' and 's' pattern (NmN.NNs)
m_count=$(printf '%s' "$line1" | tr -cd 'm' | wc -c)
s_count=$(printf '%s' "$line1" | tr -cd 's' | wc -c)
if [ "$m_count" -lt 2 ] || [ "$s_count" -lt 2 ]; then
    printf '%s\n' "FAIL: times line 1 does not have 2 time pairs: $line1" >&2
    exit 1
fi

exit 0
