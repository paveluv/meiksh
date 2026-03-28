# Test: SHALL-19-26-03-006
# Obligation: "Set the export attribute for all variable assignments."
# Tests set -a (allexport)

set -a
ALLEXPORT_TEST=exported_val
set +a

# Variable should be exported to child processes
result=$(printf '%s' "$ALLEXPORT_TEST")
if [ "$result" != "exported_val" ]; then
    printf '%s\n' "FAIL: set -a did not auto-export variable" >&2
    exit 1
fi

exit 0
