# Test: SHALL-19-03-001
# Obligation: "The shell shall read its input in terms of lines ... The input
#   lines can be of unlimited length. These lines shall be parsed using two
#   major modes: ordinary token recognition and processing of here-documents."
# Verifies: Shell can handle a long input line; both parsing modes work.

# Long line: build a 4096-char assignment and verify
long=""
i=0
while [ $i -lt 512 ]; do
    long="${long}ABCDEFGH"
    i=$(( i + 1 ))
done
eval "LONGVAR=\"$long\""
len=${#LONGVAR}
if [ "$len" -ne 4096 ]; then
    printf '%s\n' "FAIL: long line expected 4096 chars, got $len" >&2; exit 1
fi

# Here-document mode works
r=$(cat <<ENDOFHERE
hello from heredoc
ENDOFHERE
)
[ "$r" = "hello from heredoc" ] || { printf '%s\n' "FAIL: here-document" >&2; exit 1; }

exit 0
