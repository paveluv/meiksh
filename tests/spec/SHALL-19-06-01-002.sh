# Test: SHALL-19-06-01-002
# Obligation: "Otherwise, the characters in the tilde-prefix following the
#   <tilde> shall be treated as a possible login name from the user database."
# Verifies: ~username is recognized as a login name lookup.

# Use current user's login name to test ~username expansion
user=$(id -un)
result=$(eval "printf '%s\n' ~${user}")
if [ -z "$result" ] || [ "$result" = "~${user}" ]; then
    printf '%s\n' "FAIL: ~${user} did not expand (got '$result')" >&2
    exit 1
fi

# The result should be a directory path (starts with /)
case "$result" in
    /*)
        ;;
    *)
        printf '%s\n' "FAIL: ~${user} expanded to '$result' (not an absolute path)" >&2
        exit 1
        ;;
esac

exit 0
