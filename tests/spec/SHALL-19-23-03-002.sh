# Test: SHALL-19-23-03-002
# Obligation: "The export special built-in shall be a declaration utility.
#   Therefore, if export is recognized as the command name of a simple command,
#   then subsequent words of the form name=word shall be expanded in an
#   assignment context."

# Tilde expansion occurs after = in assignment context
HOME=/tmp
export EXPORT_TILDE=~/testfile
if [ "$EXPORT_TILDE" != "/tmp/testfile" ]; then
    printf '%s\n' "FAIL: tilde not expanded in export assignment context, got '$EXPORT_TILDE'" >&2
    exit 1
fi

exit 0
