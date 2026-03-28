# Test: SHALL-19-07-005
# Obligation: "A failure to open or create a file shall cause a redirection to
#   fail."
# Verifies: redirection to non-existent directory fails.

# Attempt to redirect to a path in a non-existent directory
result=$(eval 'printf "%s\n" test > /nonexistent_dir_$$_xyz/file' 2>&1) && {
    printf '%s\n' "FAIL: redirect to bad path did not fail" >&2
    exit 1
}

exit 0
