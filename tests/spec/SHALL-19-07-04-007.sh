# Test: SHALL-19-07-04-007
# Obligation: "The delimiter shall be the word itself."
# Verifies: when no quoting is applied to the here-document word, the
#   delimiter is the word as-is.

result=$(cat <<MYDELIM
hello
MYDELIM
)
if [ "$result" != "hello" ]; then
    printf '%s\n' "FAIL: unquoted delimiter MYDELIM not recognized" >&2
    printf '%s\n' "  got: $result" >&2
    exit 1
fi

exit 0
