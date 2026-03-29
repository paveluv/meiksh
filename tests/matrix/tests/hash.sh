# Test: hash — Remember or Report Utility Locations
# Target: tests/matrix/tests/hash.sh
#
# Tests the hash built-in utility for remembering, reporting, and purging
# utility locations as specified by POSIX.

. "$MATRIX_DIR/lib.sh"

# ==============================================================================
# hash -r: Forget all remembered locations
# ==============================================================================
# REQUIREMENT: SHALL-HASH-1053:
# -r option forgets all remembered locations

# After hashing a utility and then running hash -r, hash with no arguments
# should produce no output (the list is empty).
test_cmd='hash ls 2>/dev/null; hash -r; out=$(hash 2>&1); [ -z "$out" ] && echo empty || echo notempty'
assert_stdout "empty" \
    "$TARGET_SHELL -c '$test_cmd'"

# hash -r should succeed with exit code 0
assert_exit_code 0 \
    "$TARGET_SHELL -c 'hash -r'"

# ==============================================================================
# hash utility: Add to remembered locations
# ==============================================================================
# REQUIREMENT: SHALL-HASH-1054:
# utility operand adds to remembered locations list

# Hashing a known utility should succeed
assert_exit_code 0 \
    "$TARGET_SHELL -c 'hash ls'"

# After hashing a utility, hash output should mention it
test_cmd='hash -r; hash ls 2>/dev/null; hash 2>&1'
_out=$($TARGET_SHELL -c "$test_cmd" 2>&1)
case "$_out" in
    *ls*) pass ;;
    *) fail "Expected 'ls' in hash output after 'hash ls', got: $_out" ;;
esac

# Hashing a nonexistent utility should fail
assert_exit_code_non_zero \
    "$TARGET_SHELL -c 'hash nonexistent_utility_xyzzy 2>/dev/null'"

# ==============================================================================
# hash affects current shell utility location memory
# ==============================================================================
# REQUIREMENT: SHALL-HASH-1209:
# hash affects how current shell remembers utility locations

# After hashing, the shell should be able to find the utility via its
# remembered path rather than re-searching PATH.
test_cmd='hash -r; hash echo 2>/dev/null; hash 2>&1'
_out=$($TARGET_SHELL -c "$test_cmd" 2>&1)
case "$_out" in
    *echo*) pass ;;
    *) fail "Expected 'echo' in hash output after 'hash echo', got: $_out" ;;
esac

# ==============================================================================
# hash: Add locations or purge list depending on arguments
# ==============================================================================
# REQUIREMENT: SHALL-HASH-1210:
# Depending on arguments, adds locations or purges list

# With a utility argument, hash adds it
test_cmd='hash -r; hash cat 2>/dev/null; hash 2>&1'
_out=$($TARGET_SHELL -c "$test_cmd" 2>&1)
case "$_out" in
    *cat*) pass ;;
    *) fail "Expected 'cat' in hash output after 'hash cat', got: $_out" ;;
esac

# With -r, hash purges the list
test_cmd='hash cat 2>/dev/null; hash -r; out=$(hash 2>&1); [ -z "$out" ] && echo purged || echo stillhas'
assert_stdout "purged" \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# hash: Reports on contents of list when no arguments
# ==============================================================================
# REQUIREMENT: SHALL-HASH-1211:
# When no arguments, reports on contents of list

# With items in the table, hash (no args) should produce output
test_cmd='hash -r; hash ls 2>/dev/null; hash cat 2>/dev/null; out=$(hash 2>&1); [ -n "$out" ] && echo reported || echo empty'
assert_stdout "reported" \
    "$TARGET_SHELL -c '$test_cmd'"

# With an empty table, hash (no args) should produce no output
test_cmd='hash -r; out=$(hash 2>&1); [ -z "$out" ] && echo empty || echo notempty'
assert_stdout "empty" \
    "$TARGET_SHELL -c '$test_cmd'"

# Verify hash output contains paths (/ character) for hashed utilities
test_cmd='hash -r; hash ls 2>/dev/null; hash 2>&1'
_out=$($TARGET_SHELL -c "$test_cmd" 2>&1)
case "$_out" in
    */*) pass ;;
    *) fail "Expected path (containing /) in hash output, got: $_out" ;;
esac

# ==============================================================================
# hash: Built-ins and functions shall not be reported
# ==============================================================================
# REQUIREMENT: SHALL-HASH-1212:
# Built-ins and functions shall not be reported by hash

# Define a function and try to hash it; it should not appear in hash output
test_cmd='myfunc() { echo hi; }; hash -r; hash 2>&1'
_out=$($TARGET_SHELL -c "$test_cmd" 2>&1)
case "$_out" in
    *myfunc*) fail "Built-in function 'myfunc' should not appear in hash output, got: $_out" ;;
    *) pass ;;
esac

# Built-in commands like 'cd' and 'echo' (when built-in) should not appear
# in hash output unless they also exist as external utilities that were hashed.
# After hash -r, hashing only an external utility should not list built-ins.
test_cmd='hash -r; hash ls 2>/dev/null; out=$(hash 2>&1); case "$out" in *cd*) echo found_cd;; *) echo no_cd;; esac'
assert_stdout "no_cd" \
    "$TARGET_SHELL -c '$test_cmd'"

# Verify that attempting to hash a shell function does not add it to the table
test_cmd='hash -r; testfn() { :; }; hash testfn 2>/dev/null; out=$(hash 2>&1); case "$out" in *testfn*) echo found;; *) echo notfound;; esac'
assert_stdout "notfound" \
    "$TARGET_SHELL -c '$test_cmd'"

report
