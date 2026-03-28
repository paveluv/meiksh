# Test: SHALL-19-06-02-013
# Obligation: "Remove Smallest Suffix Pattern. The word shall be expanded to
#   produce a pattern. The parameter expansion shall then result in parameter,
#   with the smallest portion of the suffix matched by the pattern deleted."
# Verifies: ${parameter%word} removes smallest suffix.

x="/usr/local/bin"
result="${x%/*}"
if [ "$result" != "/usr/local" ]; then
    printf '%s\n' "FAIL: \${x%/*} gave '$result', expected '/usr/local'" >&2
    exit 1
fi

y="file.tar.gz"
result2="${y%.*}"
if [ "$result2" != "file.tar" ]; then
    printf '%s\n' "FAIL: \${y%.*} gave '$result2', expected 'file.tar'" >&2
    exit 1
fi

exit 0
