# Test: SHALL-19-06-006
# Obligation: "Tilde expansions, parameter expansions, command substitutions,
#   arithmetic expansions, and quote removals that occur within a single word
#   shall expand to a single field, except as described below."
# Verifies: expansions within a single word produce a single field (no
#   splitting in non-splitting contexts like assignments).

# Assignment context: IFS chars in value must NOT cause splitting
IFS=' '
val="one two three"
target=${val}
if [ "$target" != "one two three" ]; then
    printf '%s\n' "FAIL: assignment split '${val}' into multiple fields" >&2
    exit 1
fi

# Command substitution in assignment: single field
result=$(printf '%s %s\n' hello world)
if [ "$result" != "hello world" ]; then
    printf '%s\n' "FAIL: cmd sub in assignment did not produce single field" >&2
    exit 1
fi

# Arithmetic expansion in word yields single field
arith_result=$((10 + 20))
if [ "$arith_result" != "30" ]; then
    printf '%s\n' "FAIL: arithmetic expansion did not produce single field" >&2
    exit 1
fi

exit 0
