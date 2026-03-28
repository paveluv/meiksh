# Test: SHALL-18-02-001
# Obligation: "The values specified in Utility Limit Minimum Values represent
#   the lowest values conforming implementations shall provide ... These values
#   shall be accessible to applications via the getconf utility."
# Verifies: LINE_MAX is at least 2048 via getconf.

line_max=$(getconf LINE_MAX 2>/dev/null) || {
    printf '%s\n' "FAIL: getconf LINE_MAX failed" >&2; exit 1
}

if [ "$line_max" -lt 2048 ]; then
    printf '%s\n' "FAIL: LINE_MAX=$line_max < 2048" >&2; exit 1
fi

exit 0
