# Test: SHALL-19-09-04-05-002
# Obligation: "After the first match, no more patterns in the case statement
#   shall be expanded ... If the case statement clause is terminated by ';;',
#   no further clauses shall be examined. If the case statement clause is
#   terminated by ';&', then the compound-list (if any) of each subsequent
#   clause shall be executed"
# Verifies: ;; stops, ;& falls through.

# ;; stops further matching
result=""
case "a" in
    a) result="${result}A" ;;
    *) result="${result}B" ;;
esac
if [ "$result" != "A" ]; then
    printf '%s\n' "FAIL: ;; did not stop matching" >&2
    exit 1
fi

# ;& fall-through
result=""
case "a" in
    a) result="${result}A" ;&
    b) result="${result}B" ;&
    c) result="${result}C" ;;
    d) result="${result}D" ;;
esac
if [ "$result" != "ABC" ]; then
    printf '%s\n' "FAIL: ;& fall-through did not work: got '$result'" >&2
    exit 1
fi

exit 0
