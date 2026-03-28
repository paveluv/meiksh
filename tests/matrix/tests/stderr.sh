# Test: Standard Error Usage (Diagnostic Messages)
# Target: tests/matrix/tests/stderr.sh
#
# POSIX strictly states that standard error shall be used ONLY for diagnostic
# messages by the special built-ins, with some exceptions for job control
# and interactive prompts.

. "$MATRIX_DIR/lib.sh"

# ==============================================================================
# Clean Standard Error from Built-ins
# ==============================================================================
# REQUIREMENT: SHALL-STDERR-518: The standard error shall be used only for
# diagnostic messages.
# REQUIREMENT: SHALL-STDERR-527: The standard error shall be used only for
# diagnostic messages.
# REQUIREMENT: SHALL-STDERR-532: The standard error shall be used only for
# diagnostic messages.
# REQUIREMENT: SHALL-STDERR-536: The standard error shall be used only for
# diagnostic messages.
# REQUIREMENT: SHALL-STDERR-544: The standard error shall be used only for
# diagnostic messages.
# REQUIREMENT: SHALL-STDERR-557: The standard error shall be used only for
# diagnostic messages.
# REQUIREMENT: SHALL-STDERR-565: The standard error shall be used only for
# diagnostic messages.
# REQUIREMENT: SHALL-STDERR-574: The standard error shall be used only for
# diagnostic messages.
# REQUIREMENT: SHALL-STDERR-577: The standard error shall be used only for
# diagnostic messages.
# REQUIREMENT: SHALL-STDERR-615: The standard error shall be used only for
# diagnostic messages.
# REQUIREMENT: SHALL-STDERR-622: The standard error shall be used only for
# diagnostic messages...
# REQUIREMENT: SHALL-STDERR-627: The standard error shall be used only for
# diagnostic messages.
# REQUIREMENT: SHALL-STDERR-650: The standard error shall be used only for
# diagnostic messages and warning messages about invalid signals...
# REQUIREMENT: SHALL-STDERR-657: The standard error shall be used only for
# diagnostic messages.
# REQUIREMENT: SHALL-STDERR-030: Except as otherwise stated... standard error
# shall be used only for diagnostic messages.

# We test that all these builtins, when running successfully, produce NO
# standard error output.
test_cmd='
for i in 1; do break >/dev/null; done
for i in 1; do continue >/dev/null; done
: >/dev/null
eval "true" >/dev/null
exec 3>&1 3>&- >/dev/null
export EXPORT_VAR=1 >/dev/null
readonly RO_VAR=1 >/dev/null
set -- a b c >/dev/null
shift 1 >/dev/null
unset EXPORT_VAR >/dev/null
return 0 >/dev/null
'

assert_stderr_empty \
    "$TARGET_SHELL -c 'myfunc() { $test_cmd }; myfunc'"

report
