# Test: SHALL-19-06-01-003
# Obligation: "If the characters in the tilde-prefix following the <tilde> form
#   a portable login name, the tilde-prefix shall be replaced by a pathname of
#   the initial working directory associated with the login name."
# Verifies: ~username expands to that user's home directory.

user=$(id -un)
expected=$(eval "printf '%s\n' ~${user}")
# Cross-check: the expansion should match getent/user database
if [ -d "$expected" ]; then
    : # expanded to a valid directory
else
    printf '%s\n' "FAIL: ~${user} expanded to '$expected' which is not a directory" >&2
    exit 1
fi

# Should match $HOME for the current user
if [ "$expected" != "$HOME" ]; then
    printf '%s\n' "FAIL: ~${user} = '$expected' but HOME = '$HOME'" >&2
    exit 1
fi

exit 0
