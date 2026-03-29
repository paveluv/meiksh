#!/bin/sh

# Test: umask intrinsic utility
# Target: tests/matrix/tests/umask.sh
#
# Tests the POSIX 'umask' built-in utility.

. "$MATRIX_DIR/lib.sh"

# REQUIREMENT: SHALL-UMASK-1320: The umask utility shall conform to XBD 12.2
# Utility Syntax Guidelines .
# REQUIREMENT: SHALL-V3CHAP02-1021: The following option shall be supported: -p
# Write to standard output a list of commands associated with each condition
# operand.
# REQUIREMENT: SHALL-BG-1029: The following operand shall be supported: job_id
# Specify the job to be resumed as a background job.
# REQUIREMENT: SHALL-UMASK-1324: For a symbolic_mode value, the new value of the
# file mode creation mask shall be the logical complement of the file permission
# bits portion of the file mode specified by the symbolic_mode string.
# REQUIREMENT: SHALL-UMASK-1325: In a symbolic_mode value, the permissions op
# characters '+' and '-' shall be interpreted relative to the current file mode
# creation mask; '+' shall cause the bits for the indicated permissions to be
# cleared in the mask; '-' shall cause the bits for the indicated permissions to
# be set in the mask.
# REQUIREMENT: SHALL-UMASK-1326: The file mode creation mask shall be set to the
# resulting numeric value.
# REQUIREMENT: SHALL-UMASK-1329: When the mask operand is not specified, the
# umask utility shall write a message to standard output that can later be used
# as a umask mask operand.
# REQUIREMENT: SHALL-UMASK-1331: If -S is specified, the message shall be in the
# following format: "u=%s,g=%s,o=%s\n", < owner permissions >, < group
# permissions >, < other permissions > where the three values shall be
# combinations of letters from the set { r , w , x }; the presence of a letter
# shall indicate that the corresponding bit is clear in the file mode creation
# mask.
# REQUIREMENT: SHALL-UMASK-1331: If -S is specified, the message shall be in the
# following format: "u=%s,g=%s,o=%s\n", < owner permissions >, < group
# permissions >, < other permissions > where the three values shall be
# combinations of letters from the set { r , w , x }; the presence of a letter
# shall indicate that the corresponding bit is clear in the file mode creation
# mask.

test_cmd='
    umask 022
    val=$(umask)
    # verify format (typically 0022 or 022)
    echo "$val" | grep -q "022" && echo "pass" || echo "fail: $val"
'
assert_stdout "pass" "$TARGET_SHELL -c '$test_cmd'"

test_cmd='
    umask 022
    val=$(umask -S)
    echo "$val"
'
assert_stdout "u=rwx,g=rx,o=rx" "$TARGET_SHELL -c '$test_cmd'"

test_cmd='
    umask 022
    umask a=rx
    val=$(umask)
    echo "$val" | grep -q "222" && echo "pass" || echo "fail: $val"
'
assert_stdout "pass" "$TARGET_SHELL -c '$test_cmd'"

test_cmd='
    umask 022
    umask g-w
    val=$(umask -S)
    echo "$val"
'
assert_stdout "u=rwx,g=rx,o=rx" "$TARGET_SHELL -c '$test_cmd'"

report
