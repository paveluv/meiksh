# Test: Extended umask Built-in
# Target: tests/matrix/tests/umask_extended.sh
#
# Tests POSIX requirements for umask: setting and reporting file mode creation
# mask, symbolic output, subshell isolation, default output round-trip.

. "$MATRIX_DIR/lib.sh"

# ==============================================================================
# umask sets file mode creation mask of current shell
# ==============================================================================
# REQUIREMENT: SHALL-UMASK-1316:
# umask shall set the file mode creation mask of the current shell
# execution environment.

assert_exit_code 0 "$TARGET_SHELL -c 'umask 0022'"

# Verify the mask is applied
assert_stdout "0022" "$TARGET_SHELL -c 'umask 0022; umask'"

# ==============================================================================
# Mask affects initial permission bits of new files
# ==============================================================================
# REQUIREMENT: SHALL-UMASK-1317:
# The file mode creation mask shall affect the initial value of the file
# permission bits of subsequently created files.

_result=$($TARGET_SHELL -c '
    _d=$(mktemp -d)
    umask 0077
    > "$_d/test_077"
    umask 0000
    > "$_d/test_000"
    ls -l "$_d/test_077" "$_d/test_000"
    rm -rf "$_d"
')
_perm_077=$(echo "$_result" | head -1 | awk '{print $1}')
_perm_000=$(echo "$_result" | tail -1 | awk '{print $1}')
if [ "$_perm_077" != "$_perm_000" ]; then
    pass
else
    fail "umask did not affect file permissions: 077=$_perm_077, 000=$_perm_000"
fi

# ==============================================================================
# If no mask operand, write current mask value to stdout
# ==============================================================================
# REQUIREMENT: SHALL-UMASK-1319:
# If the mask operand is not specified, the umask utility shall write to
# standard output the value of the file mode creation mask.

_out=$($TARGET_SHELL -c 'umask 0027; umask')
if [ -n "$_out" ]; then
    pass
else
    fail "umask with no operand produced no output"
fi

# Verify it reflects what was set
assert_stdout "0027" "$TARGET_SHELL -c 'umask 0027; umask'"

# ==============================================================================
# Exit 0 on success or no mask operand
# ==============================================================================
# REQUIREMENT: SHALL-UMASK-1082:
# The exit status shall be 0 if the mask operand was successfully set
# or if no mask operand was supplied.

assert_exit_code 0 "$TARGET_SHELL -c 'umask 0022'"
assert_exit_code 0 "$TARGET_SHELL -c 'umask'"

# ==============================================================================
# mask operand specifies new file mode creation mask
# ==============================================================================
# REQUIREMENT: SHALL-UMASK-1080:
# The mask operand shall specify the new file mode creation mask value.

# Octal form
assert_stdout "0037" "$TARGET_SHELL -c 'umask 0037; umask'"

# Symbolic form
_out2=$($TARGET_SHELL -c 'umask u=rwx,g=rx,o=; umask')
case "$_out2" in
    0007|007) pass ;;
    *) fail "umask symbolic assignment produced unexpected mask: '$_out2'" ;;
esac

# ==============================================================================
# -S option produces symbolic output
# ==============================================================================
# REQUIREMENT: SHALL-UMASK-1079:
# -S: Produce symbolic output.

_sym=$($TARGET_SHELL -c 'umask 0022; umask -S')
case "$_sym" in
    *u=*g=*o=*) pass ;;
    *) fail "umask -S did not produce symbolic output: '$_sym'" ;;
esac

# Verify -S reflects the correct mask
_sym2=$($TARGET_SHELL -c 'umask 0077; umask -S')
case "$_sym2" in
    *u=rwx*g=*o=*) pass ;;
    *) fail "umask -S for 0077 unexpected: '$_sym2'" ;;
esac

# With mask 0000 all permissions should be shown
_sym3=$($TARGET_SHELL -c 'umask 0000; umask -S')
case "$_sym3" in
    *u=rwx*g=rwx*o=rwx*) pass ;;
    *) fail "umask -S for 0000 unexpected: '$_sym3'" ;;
esac

# ==============================================================================
# umask in subshell shall not affect caller
# ==============================================================================
# REQUIREMENT: SHALL-UMASK-1318:
# The file mode creation mask set in a subshell shall not affect the
# parent (caller) shell execution environment.

assert_stdout "0022" \
    "$TARGET_SHELL -c 'umask 0022; (umask 0077); umask'"

# Verify the subshell actually ran a different mask
_both=$($TARGET_SHELL -c 'umask 0022; (umask 0077; echo inner=$(umask)); echo outer=$(umask)')
case "$_both" in
    *inner=0077*outer=0022*) pass ;;
    *) fail "subshell umask affected parent: '$_both'" ;;
esac

# ==============================================================================
# Default output recognized on subsequent invocation as mask
# ==============================================================================
# REQUIREMENT: SHALL-UMASK-1322:
# When the user invokes umask with no operand, the output shall be in a
# form that is recognized on a subsequent invocation as a mask operand.

_roundtrip=$($TARGET_SHELL -c '
    umask 0037
    _saved=$(umask)
    umask 0000
    umask "$_saved"
    umask
')
if [ "$_roundtrip" = "0037" ]; then
    pass
else
    fail "Default output round-trip failed: expected '0037', got '$_roundtrip'"
fi

# ==============================================================================
# Default output recognized as mask operand
# ==============================================================================
# REQUIREMENT: SHALL-UMASK-1327:
# The default output shall be recognized as a mask operand.

# Another round-trip with a different mask
_roundtrip2=$($TARGET_SHELL -c '
    umask 0055
    _m=$(umask)
    umask 0000
    umask "$_m"
    umask
')
if [ "$_roundtrip2" = "0055" ]; then
    pass
else
    fail "Default output as mask operand failed: expected '0055', got '$_roundtrip2'"
fi

# ==============================================================================
# If mask operand specified, no output to stdout
# ==============================================================================
# REQUIREMENT: SHALL-UMASK-1332:
# If the mask operand is specified, there shall be no output written to
# standard output.

_set_out=$($TARGET_SHELL -c 'umask 0022')
if [ -z "$_set_out" ]; then
    pass
else
    fail "umask with operand produced stdout: '$_set_out'"
fi

# Also test symbolic form produces no output when setting
_set_sym_out=$($TARGET_SHELL -c 'umask u=rwx,g=rx,o=rx')
if [ -z "$_set_sym_out" ]; then
    pass
else
    fail "umask with symbolic operand produced stdout: '$_set_sym_out'"
fi

report
