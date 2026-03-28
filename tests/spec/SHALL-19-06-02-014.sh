# Test: SHALL-19-06-02-014
# Obligation: "Remove Largest Suffix Pattern. The word shall be expanded to
#   produce a pattern. The parameter expansion shall then result in parameter,
#   with the largest portion of the suffix matched by the pattern deleted."
# Verifies: ${parameter%%word} removes largest suffix.

x="/usr/local/bin"
result="${x%%/*}"
if [ "$result" != "" ]; then
    printf '%s\n' "FAIL: \${x%%/*} gave '$result', expected ''" >&2
    exit 1
fi

y="file.tar.gz"
result2="${y%%.*}"
if [ "$result2" != "file" ]; then
    printf '%s\n' "FAIL: \${y%%.*} gave '$result2', expected 'file'" >&2
    exit 1
fi

exit 0
