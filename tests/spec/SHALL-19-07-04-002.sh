# Test: SHALL-19-07-04-002
# Obligation: "If any part of word is quoted [...] the delimiter shall be
#   formed by performing quote removal on word, and the here-document lines
#   shall not be expanded."
# Verifies: quoted delimiter suppresses expansion in here-doc body.

HOME=/should_not_appear
result=$(cat <<'EOF'
$HOME
EOF
)
if [ "$result" != '$HOME' ]; then
    printf '%s\n' "FAIL: quoted here-doc delimiter did not suppress expansion: got '$result'" >&2
    exit 1
fi

# Backslash-quoted delimiter also suppresses expansion
result2=$(cat <<\END
$HOME
END
)
if [ "$result2" != '$HOME' ]; then
    printf '%s\n' "FAIL: backslash-quoted delimiter did not suppress: got '$result2'" >&2
    exit 1
fi

exit 0
