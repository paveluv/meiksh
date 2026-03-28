# Test: SHALL-20-110-08-011
# Obligation: "Determine the pathname of the user's home directory. The
#   contents of HOME are used in tilde expansion as described in 2.6.1
#   Tilde Expansion."
# Verifies: HOME is used for tilde expansion.

result=$("$MEIKSH" -c 'HOME=/tmp/fakehome; printf "%s\n" ~')
if [ "$result" != "/tmp/fakehome" ]; then
    printf '%s\n' "FAIL: tilde did not expand to HOME, got '$result'" >&2
    exit 1
fi

exit 0
