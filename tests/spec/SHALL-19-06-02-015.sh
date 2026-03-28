# Test: SHALL-19-06-02-015
# Obligation: "Remove Smallest Prefix Pattern. The word shall be expanded to
#   produce a pattern. The parameter expansion shall then result in parameter,
#   with the smallest portion of the prefix matched by the pattern deleted."
# Verifies: ${parameter#word} removes smallest prefix.

x="/usr/local/bin"
result="${x#*/}"
if [ "$result" != "usr/local/bin" ]; then
    printf '%s\n' "FAIL: \${x#*/} gave '$result', expected 'usr/local/bin'" >&2
    exit 1
fi

y="file.tar.gz"
result2="${y#*.}"
if [ "$result2" != "tar.gz" ]; then
    printf '%s\n' "FAIL: \${y#*.} gave '$result2', expected 'tar.gz'" >&2
    exit 1
fi

exit 0
