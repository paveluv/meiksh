# Test: Simple Commands
# Target: tests/matrix/tests/simple_commands.sh
#
# Simple commands are the workhorse of the shell. Here we verify that the shell
# processes variable assignments, arguments, redirections, and command search
# rules in the exact order POSIX dictates.

. "$MATRIX_DIR/lib.sh"

# ==============================================================================
# Order of Processing
# ==============================================================================
# REQUIREMENT: SHALL-2-9-1-1-267: The first word (if any) that is not a variable
# assignment or redirection shall be expanded.
# REQUIREMENT: SHALL-2-9-1-1-268: If any fields remain following its expansion,
# the first field shall be considered the command name.

# We test that a variable expands into the command name and its first argument.
test_cmd='cmd="printf %s\n"; $cmd "hello"'
assert_stdout 'hello' \
    "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-2-9-1-1-269: If no fields remain, the next word (if any)
# shall be expanded, and so on, until a command name is found.

# If the first word expands to nothing, the shell must keep looking.
test_cmd='empty=""; $empty printf "%s\n" "hello"'
assert_stdout 'hello' \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# Variable Assignments
# ==============================================================================
# REQUIREMENT: SHALL-2-9-1-2-276: If no command name results, variable
# assignments shall affect the current execution environment.

# A simple assignment with no command name permanently alters the shell state.
test_cmd='FOO=bar; printf "%s\n" "$FOO"'
assert_stdout 'bar' \
    "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-2-9-1-2-277: If the command name is not a special built-in
# utility or function, the variable assignments shall be exported for the
# execution environment of the command and shall not affect the current
# execution environment...

# Here, FOO is set only for `sh`, and should not persist afterward.
test_cmd='FOO=bar sh -c "printf \"%s\n\" \"\$FOO\""; printf "%s\n" "${FOO:-unset}"'
assert_stdout 'bar
unset' \
    "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-2-9-1-2-279: If the command name is a special built-in
# utility, variable assignments shall affect the current execution environment...

# `export` is a special built-in. A preceding variable assignment should persist!
# (Note: POSIX specifies that assignments before special built-ins persist).
test_cmd='FOO=bar export DUMMY=1; printf "%s\n" "$FOO"'
assert_stdout 'bar' \
    "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-2-9-1-2-281: If any of the variable assignments attempt to
# assign a value to a variable for which the readonly attribute is set...

# Assigning to a readonly variable must fail.
test_cmd='readonly FOO=1; FOO=2'
assert_exit_code_non_zero \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# Redirections without Command Names
# ==============================================================================
# REQUIREMENT: SHALL-2-9-1-3-282: If a simple command has no command name after
# word expansion... any redirections shall be performed in a subshell...
# REQUIREMENT: SHALL-2-9-1-3-283: (To affect the current execution environment,
# see the exec special built-in.) If any of the redirections fail, the command
# shall exit with a non-zero status.

# A redirection with no command truncates/creates the file, but doesn't run a
# command.
test_cmd='> tmp_redir.txt; ls tmp_redir.txt'
assert_stdout 'tmp_redir.txt' \
    "$TARGET_SHELL -c '$test_cmd'"

# A failed redirection with no command name should yield non-zero.
test_cmd='> /does_not_exist/file'
assert_exit_code_non_zero \
    "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-2-9-1-3-284: Additionally, if there is no command name but
# the command contains a command substitution, the command shall complete with
# the exit status of the last command substitution performed.

test_cmd='var=$(false)'
assert_exit_code_non_zero \
    "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-2-9-1-3-285: Otherwise, the command shall complete with a
# zero exit status.

test_cmd='FOO=bar'
assert_exit_code 0 \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# Command Search and Execution
# ==============================================================================
# REQUIREMENT: SHALL-2-9-1-4-288: If the command name matches the name of a
# function known to this shell, the function shall be invoked...

test_cmd='myfunc() { printf "%s\n" "in func"; }; myfunc'
assert_stdout 'in func' \
    "$TARGET_SHELL -c '$test_cmd'"

report
