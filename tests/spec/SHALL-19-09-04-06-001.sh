# Test: SHALL-19-09-04-06-001
# Obligation: "The exit status of case shall be zero if no patterns are matched.
#   Otherwise, the exit status shall be the exit status of the compound-list
#   of the last clause to be executed."
# Verifies: case exit status.

case "nomatch" in
    xyz) true ;;
esac
rc=$?
if [ "$rc" -ne 0 ]; then
    printf '%s\n' "FAIL: no-match case should exit 0, got $rc" >&2
    exit 1
fi

case "a" in
    a) false ;;
esac
rc=$?
if [ "$rc" -ne 1 ]; then
    printf '%s\n' "FAIL: matched case with false should exit 1, got $rc" >&2
    exit 1
fi

exit 0
