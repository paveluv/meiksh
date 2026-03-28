# Test: SHALL-19-09-04-05-001
# Obligation: "The conditional construct case shall execute the compound-list
#   corresponding to the first pattern ... that is matched by the string
#   resulting from the tilde expansion, parameter expansion, command
#   substitution, arithmetic expansion, and quote removal of the given word."
# Verifies: case matches first pattern; word undergoes expansion.

V=hello
result=""
case $V in
    hello) result="matched" ;;
    *) result="nomatch" ;;
esac
if [ "$result" != "matched" ]; then
    printf '%s\n' "FAIL: case did not match expanded word" >&2
    exit 1
fi

# Multiple patterns with |
result=""
case "b" in
    a|b|c) result="matched" ;;
    *) result="nomatch" ;;
esac
if [ "$result" != "matched" ]; then
    printf '%s\n' "FAIL: case with | patterns did not match" >&2
    exit 1
fi

exit 0
