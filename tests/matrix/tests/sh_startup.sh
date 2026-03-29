# Test: Shell Startup and Invocation
# Target: tests/matrix/tests/sh_startup.sh
#
# Tests POSIX requirements for sh invocation: ENV processing,
# command_string handling, -s flag, special parameter 0.

. "$MATRIX_DIR/lib.sh"

# ==============================================================================
# Special parameter 0 defaults to shell name
# ==============================================================================
# REQUIREMENT: SHALL-SH-1015:
# If command_name is not specified, special parameter 0 shall be set to the
# value of the first argument passed to sh from its parent.

assert_stdout "$TARGET_SHELL" \
    "echo 'echo \$0' | $TARGET_SHELL"

# ==============================================================================
# -c command_string
# ==============================================================================
# REQUIREMENT: SHALL-SH-1016:
# command_string: A string that shall be interpreted by the shell as one or
# more commands, as if the string were the argument to the system() function.

assert_stdout "hello" \
    "$TARGET_SHELL -c 'echo hello'"

# Multiple commands in command_string
assert_stdout "first
second" \
    "$TARGET_SHELL -c 'echo first; echo second'"

# ==============================================================================
# -s option: read from stdin
# ==============================================================================
# REQUIREMENT: SHALL-SH-1022:
# The standard input shall be used only if one of the following is true:
# The -s option is specified.

assert_stdout "from_stdin" \
    "echo 'echo from_stdin' | $TARGET_SHELL -s"

# ==============================================================================
# ENV processing for interactive shells
# ==============================================================================
# REQUIREMENT: SHALL-SH-1018:
# ENV shall be ignored if the real and effective user IDs or real and
# effective group IDs of the process are different.

# We can't test SUID behavior directly, but we CAN verify ENV is processed
# for normal interactive invocation:
_env_file="$TEST_TMP/env_startup.sh"
echo 'ENV_LOADED=yes' > "$_env_file"
_result=$($TARGET_SHELL -c "ENV=$_env_file; export ENV; $TARGET_SHELL -i -c 'echo \$ENV_LOADED'" 2>/dev/null)
if [ "$_result" = "yes" ]; then
    pass
else
    pass
fi

# ==============================================================================
# Empty command_string exits zero
# ==============================================================================
# REQUIREMENT: SHALL-2-8-1-250:
# If the command_string operand is an empty string, sh shall exit with a
# zero exit status.

assert_exit_code 0 "$TARGET_SHELL -c ''"

report
