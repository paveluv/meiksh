# Test: SHALL-19-24-03-002
# Obligation: "The readonly special built-in shall be a declaration utility.
#   Therefore, if readonly is recognized as the command name of a simple command,
#   then subsequent words of the form name=word shall be expanded in an
#   assignment context."

# Tilde expansion in assignment context
HOME=/tmp
readonly RO_TILDE=~/testfile
if [ "$RO_TILDE" != "/tmp/testfile" ]; then
    printf '%s\n' "FAIL: tilde not expanded in readonly assignment context" >&2
    exit 1
fi

exit 0
