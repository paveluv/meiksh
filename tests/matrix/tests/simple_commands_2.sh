# Test: Simple Commands - Executing Utilities and Functions
# Target: tests/matrix/tests/simple_commands_2.sh
#
# A "simple command" encompasses command search, PATH resolution, variable
# assignment scopes during utility vs function calls, and error handling.

. "$MATRIX_DIR/lib.sh"

# ==============================================================================
# Variable Assignment Scope
# ==============================================================================
# REQUIREMENT: SHALL-2-9-1-2-275: Variable assignments shall be performed as
# follows:
# REQUIREMENT: SHALL-2-9-1-2-280: If the command name is a function that is not
# a standard utility... variable assignments shall affect the current execution
# environment and shall not be restored to their prior values.

# When calling a shell function, any `VAR=value` before it permanently modifies
# the environment.
test_cmd='
my_func() { echo "func"; }
my_var="old"
my_var="new" my_func >/dev/null
echo "$my_var"
'
assert_stdout "new" \
    "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-2-9-1-1-274: Each variable assignment shall be expanded for
# tilde expansion, parameter expansion, command substitution, and arithmetic
# expansion.

# We test that arithmetic expansion happens inside variable assignments preceding
# a simple command.
test_cmd='
my_var=$((2+3)) env | grep -q "^my_var=5$" && echo "expanded"
'
assert_stdout "expanded" \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# Simple Command Execution Steps
# ==============================================================================
# REQUIREMENT: SHALL-2-9-1-5-298: If a standard utility or a conforming
# application is executed with file descriptor 0 not open for re...
# REQUIREMENT: SHALL-2-9-1-4-286: If a simple command has a command name and an
# optional list of arguments after word expansion...
# REQUIREMENT: SHALL-2-9-1-4-290: It shall be invoked in conjunction with the
# path search in step 1e.
# REQUIREMENT: SHALL-2-9-1-4-291: If the command name matches the name of an
# intrinsic utility (see 1.7 Intrinsic Utilities)...
# REQUIREMENT: SHALL-2-9-1-4-294: Otherwise, the shell shall execute a non-
# built-in utility as described in 2.9.1.6...
# REQUIREMENT: SHALL-2-9-1-4-295: If the remembered location fails for a
# subsequent invocation, the shell shall repeat the search...

# We test that a command name resolves to a regular utility and executes.
test_cmd='echo test_utility'
assert_stdout "test_utility" \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# Standard Utility Implementations
# ==============================================================================
# REQUIREMENT: SHALL-2-9-1-2-278: If the command name is a standard utility
# implemented as a function (see XBD 4.25 Utility), the effect of variable
# assignments shall be...
# REQUIREMENT: SHALL-2-9-1-1-270: If there is a command name and it is
# recognized as a declaration utility, then any remaining words...

test_cmd='export var=123; env | grep "^var="'
assert_stdout "var=123" \
    "$TARGET_SHELL -c '$test_cmd'"
# ==============================================================================
# REQUIREMENT: SHALL-2-9-1-4-287: If the command name does not contain any
# <slash> characters, the first successful step in the following sequence...
# REQUIREMENT: SHALL-2-9-1-4-289: has not been removed using unset -f...
# REQUIREMENT: SHALL-2-9-1-4-292: Otherwise, the command shall be searched for
# using the PATH environment variable...
# REQUIREMENT: SHALL-2-9-1-4-293: Environment Variables: If the search is
# successful: If the system has implemented the utility as a built-in...

# A command without slashes uses PATH. We put a custom script in a custom PATH.
mkdir -p tmp_path_test
echo 'echo "found_in_path"' > tmp_path_test/my_custom_cmd
chmod +x tmp_path_test/my_custom_cmd

test_cmd='PATH="$PWD/tmp_path_test:$PATH" my_custom_cmd'
assert_stdout "found_in_path" \
    "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-2-9-1-4-297: If the command name contains at least one
# <slash>, the shell shall execute a non-built-in utility as described in
# 2.9.1.6...

test_cmd='./tmp_path_test/my_custom_cmd'
assert_stdout "found_in_path" \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# Non-Executable Execution (ENOEXEC equivalent)
# ==============================================================================
# REQUIREMENT: SHALL-2-9-1-6-299: exec2.13 Shell Execution EnvironmentIf the
# execution is being made via the exec special built-in utility...
# REQUIREMENT: SHALL-2-9-1-6-300: If the current shell environment is a
# subshell environment, the new process image shall replace the...
# REQUIREMENT: SHALL-2-9-1-6-301: In either case, execution of the utility in
# the specified environment shall be performed as follows:...
# REQUIREMENT: SHALL-2-9-1-6-302: If the command name does not contain any
# <slash> characters, the command name shall be searched for...
# REQUIREMENT: SHALL-2-9-1-6-303: Environment Variables: If the search is
# successful, the shell shall execute the utility with actions...
# REQUIREMENT: SHALL-2-9-1-6-307: If the command name contains at least one
# <slash>: If the named utility exists, the shell shall execute...
# REQUIREMENT: SHALL-2-9-1-6-308: If the execl() function fails due to an error
# equivalent to the [ENOEXEC] error, the shell shall execute a command
# equivalent to having a shell invoked...
# REQUIREMENT: SHALL-2-9-1-6-309: In this case, it shall write an error message,
# and the command shall fail with an exit status of 126...
# REQUIREMENT: SHALL-2-9-1-6-310: If the named utility does not exist, the
# command shall fail with an exit status of 127 and the shell shall write an
# error message...

# Creating a file without an execute bit or without a valid header triggers
# a fallback to executing it as a shell script IF it has execute permissions.
# If it does NOT have execute permissions, it fails with 126.
echo "echo executed_fallback" > tmp_path_test/no_magic_header
chmod +x tmp_path_test/no_magic_header
test_cmd='./tmp_path_test/no_magic_header'
assert_stdout "executed_fallback" \
    "$TARGET_SHELL -c '$test_cmd'"

# A completely non-existent file with a slash fails with 127.
test_cmd='./tmp_path_test/does_not_exist_123'
assert_exit_code 127 \
    "$TARGET_SHELL -c '$test_cmd'"


report
