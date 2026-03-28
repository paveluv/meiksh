# Test: SHALL-19-06-01-001
# Obligation: "If the tilde-prefix consists of only the <tilde> character, it
#   shall be replaced by the value of the variable HOME."
# Verifies: ~ expands to $HOME.

saved_home="$HOME"
HOME=/tmp/test_tilde_home
result=$(printf '%s\n' ~)
if [ "$result" != "/tmp/test_tilde_home" ]; then
    printf '%s\n' "FAIL: ~ expanded to '$result', expected '/tmp/test_tilde_home'" >&2
    HOME="$saved_home"
    exit 1
fi

# ~ followed by / should also replace the tilde part
result2=$(printf '%s\n' ~/subdir)
if [ "$result2" != "/tmp/test_tilde_home/subdir" ]; then
    printf '%s\n' "FAIL: ~/subdir expanded to '$result2'" >&2
    HOME="$saved_home"
    exit 1
fi

HOME="$saved_home"
exit 0
