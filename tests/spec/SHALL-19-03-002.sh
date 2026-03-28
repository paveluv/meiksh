# Test: SHALL-19-03-002
# Obligation: "When an io_here token has been recognized by the grammar,
#   one or more of the subsequent lines immediately following the next
#   NEWLINE token form the body of a here-document and shall be parsed
#   according to the rules of 2.7.4 Here-Document."
# Verifies: Here-document body is parsed after NEWLINE; tokens after <<
#   on the same line are saved and processed after the here-doc.

# Tokens after << on same line are processed after here-doc
result=$(cat <<EOF; printf '%s' ' world'
hello
EOF
)
[ "$result" = "hello world" ] || {
    printf '%s\n' "FAIL: saved tokens after here-doc, got '$result'" >&2; exit 1
}

# Chained here-documents
r1=$(cat <<A
first
A
)
r2=$(cat <<B
second
B
)
[ "$r1" = "first" ] || { printf '%s\n' "FAIL: chained heredoc 1" >&2; exit 1; }
[ "$r2" = "second" ] || { printf '%s\n' "FAIL: chained heredoc 2" >&2; exit 1; }

exit 0
