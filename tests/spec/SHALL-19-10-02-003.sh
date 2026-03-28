# Test: SHALL-19-10-02-003
# Obligation: "Quote removal shall be applied to the word to determine the
#   delimiter that is used to find the end of the here-document"
# Verifies: Here-document delimiter quote removal; quoted delimiter suppresses expansion.

# Unquoted delimiter: expansion occurs
V=hello
result=$(cat <<DELIM
$V
DELIM
)
if [ "$result" != "hello" ]; then
    printf '%s\n' "FAIL: unquoted here-doc delimiter should allow expansion" >&2
    exit 1
fi

# Quoted delimiter: no expansion
result=$(cat <<'DELIM'
$V
DELIM
)
if [ "$result" != '$V' ]; then
    printf '%s\n' "FAIL: quoted here-doc delimiter should suppress expansion" >&2
    exit 1
fi

exit 0
