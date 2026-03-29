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
# REQUIREMENT: SHALL-STDERR-518:
# The standard error shall be used only for diagnostic messages.
# REQUIREMENT: SHALL-STDERR-518:
# The standard error shall be used only for diagnostic messages.
# REQUIREMENT: SHALL-STDERR-518:
# The standard error shall be used only for diagnostic messages.
# REQUIREMENT: SHALL-STDERR-518:
# The standard error shall be used only for diagnostic messages.
# REQUIREMENT: SHALL-STDERR-518:
# The standard error shall be used only for diagnostic messages.
# REQUIREMENT: SHALL-STDERR-557:
# If the resource being reported does not have a numeric limit, in the POSIX
# locale the following format shall be used: "unlimited\n" STDERR The standard
# error shall be used only for diagnostic messages.
# REQUIREMENT: SHALL-STDERR-518:
# The standard error shall be used only for diagnostic messages.
# REQUIREMENT: SHALL-STDERR-574:
# When the -V option is specified, standard output shall be formatted as:
# "%s\n", < unspecified > STDERR The standard error shall be used only for
# diagnostic messages.
# REQUIREMENT: SHALL-STDERR-577:
# If the < command > consists of more than one line, the lines after the first
# shall be displayed as: "\t%s\n", < continued-command > STDERR The standard
# error shall be used only for diagnostic messages.
# messages.
# REQUIREMENT: SHALL-STDERR-518:
# The standard error shall be used only for diagnostic messages.
# REQUIREMENT: SHALL-STDERR-622:
# The standard error shall be used only for diagnostic messages and the warning
# message specified in EXIT STATUS.
# REQUIREMENT: SHALL-STDERR-627:
# The fg utility shall write the command line of the job to standard output in
# the following format: "%s\n", < command > STDERR The standard error shall be
# used only for diagnostic messages.
# REQUIREMENT: SHALL-STDERR-650:
# The standard error shall be used only for diagnostic messages and warning
# messages about invalid signal names XSI
# REQUIREMENT: SHALL-STDERR-518:
# The standard error shall be used only for diagnostic messages.
# REQUIREMENT: SHALL-STDERR-030:
# Except as otherwise stated (by the descriptions of any invoked utilities or
# in interactive mode), standard error shall be used only for diagnostic
# messages.

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
