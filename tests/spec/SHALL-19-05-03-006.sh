# SHALL-19-05-03-006
# "HOME ... The pathname of the user's home directory. The contents of HOME are
#  used in tilde expansion."
# Verify HOME is used for tilde expansion.

fail=0

# Set HOME and verify ~ expands to it
HOME=/tmp/fakehome
result=$(eval 'printf "%s" ~')
[ "$result" = "/tmp/fakehome" ] || { printf '%s\n' "FAIL: ~ expanded to '$result', expected /tmp/fakehome" >&2; fail=1; }

exit "$fail"
