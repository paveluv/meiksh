# Test: SHALL-19-07-04-006
# Obligation: "Any <backslash> characters in the input shall behave as the
#   <backslash> inside double-quotes (see 2.2.3 Double-Quotes). However,
#   the double-quote character shall not be treated specially within a
#   here-document, except when the double-quote appears within \"$()\",
#   \"\`\`\", or \"${}\"."
# Verifies: backslash is special only before $, `, ", \, newline in heredoc
#   body; double-quote is literal in heredoc.

result=$(cat <<EOF
\$literal_dollar
\\backslash
\not_special
"literal_quotes"
EOF
)
expected='$literal_dollar
\backslash
\not_special
"literal_quotes"'
if [ "$result" != "$expected" ]; then
    printf '%s\n' "FAIL: heredoc backslash/quote handling incorrect" >&2
    printf '%s\n' "  expected: $expected" >&2
    printf '%s\n' "  got:      $result" >&2
    exit 1
fi

exit 0
