# Test: SHALL-19-06-02-016
# Obligation: "Remove Largest Prefix Pattern. The word shall be expanded to
#   produce a pattern. The parameter expansion shall then result in parameter,
#   with the largest portion of the prefix matched by the pattern deleted."
# Verifies: ${parameter##word} removes largest prefix.

x="/usr/local/bin"
result="${x##*/}"
if [ "$result" != "bin" ]; then
    printf '%s\n' "FAIL: \${x##*/} gave '$result', expected 'bin'" >&2
    exit 1
fi

y="file.tar.gz"
result2="${y##*.}"
if [ "$result2" != "gz" ]; then
    printf '%s\n' "FAIL: \${y##*.} gave '$result2', expected 'gz'" >&2
    exit 1
fi

exit 0
