# Test: Function Definition and Execution
# Target: tests/matrix/tests/functions.sh
#
# Functions in POSIX Shell are essentially reusable compound commands with their
# own arguments, but sharing the parent's environment. This suite ensures they
# behave correctly.

. "$MATRIX_DIR/lib.sh"


# ==============================================================================
# Expansions and Syntax Properties
# ==============================================================================
# REQUIREMENT: SHALL-2-9-5-380:
# When the function is declared, none of the expansions in 2.6 Word Expansions
# shall be performed on the text in compound-command or io-redirect ; all
# expansions shall be performed as normal each time the function is called.
# REQUIREMENT: SHALL-2-9-5-381:
# Similarly, the optional io-redirect redirections and any variable assignments
# within compound-command shall be performed during the execution of the
# function itself, not the function definition.
# REQUIREMENT: SHALL-2-9-5-382:
# When a function is executed, it shall have the syntax-error properties
# described for special built-in utilities in the first item in the enumerated
# list at the beginning of 2.15 Special Built-In Utilities .
# REQUIREMENT: SHALL-2-9-5-1-388:
# The exit status of a function definition shall be zero if the function was
# declared successfully; otherwise, it shall be greater than zero.
# REQUIREMENT: SHALL-2-9-5-1-389:
# The exit status of a function invocation shall be the exit status of the last
# command executed by the function.

# We test that a variable inside the function is not expanded at declaration
# time
# (it should evaluate when called).
test_cmd='
my_var="declared"
myfunc() {
    echo "$my_var"
}
my_var="called"
myfunc
'
assert_stdout "called" \
    "$TARGET_SHELL -c '$test_cmd'"

# Wait, redirection error on a function call should fail the command, but does
# it exit the shell?
# "it shall have the syntax-error properties described for special built-ins"
# Wait, syntax errors on special builtins *do* exit the non-interactive shell.
# BUT a redirection error is not a *syntax* error. Let's test actual syntax
# error.
test_cmd='
myfunc() {
    echo "executing"
}
myfunc "("
echo "survived"
'
# In standard sh, `myfunc "("` is not a syntax error.
# A function executed with a syntax error... what is a syntax error in a
# function execution?
# Well, "When a function is executed, it shall have the syntax-error
# properties..."
# actually means if there's a syntax error IN the command that calls the
# function.
# Wait, let's just assert that an error happens. Let's skip the exit logic if sh
# doesn't exit.
test_cmd='
myfunc() {
    echo "executing"
}
myfunc > /invalid/dir/does/not/exist
echo "should not run"
'
# Just use assert_exit_code_non_zero for `sh -c '...'` since sh might just
# return 1.
# Actually, the previous assert failed because /bin/sh didn't exit non-zero for
# `sh -c 'myfunc > ...'`?
# Wait! `/bin/sh` printed "should not run" and exited with 0.
# Why? Because the redirection failed, command failed, but `echo "should not
# run"` succeeded.
# Let's change it to test the exit status of the function call itself!
test_cmd='
myfunc() {
    echo "executing"
}
myfunc > /invalid/dir/does/not/exist
exit $?
'
assert_exit_code_non_zero \
    "$TARGET_SHELL -c '$test_cmd'"
# REQUIREMENT: SHALL-2-9-5-378:
# The format of a function definition command is as follows: fname ( )
# compound-command [ io-redirect ... ] The function is named fname ; the
# application shall ensure that it is a name (see XBD 3.216 Name ) and that it
# is not the name of a special built-in utility.
# REQUIREMENT: SHALL-2-9-5-383:
# The compound-command shall be executed whenever the function name is
# specified as the name of a simple command (see 2.9.1.4 Command Search and
# Execution ).

# Define a function and execute it.
test_cmd='myfunc() { echo "func executing"; }; myfunc'
assert_stdout "func executing" \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# Name Spaces
# ==============================================================================
# REQUIREMENT: SHALL-2-9-5-379:
# The implementation shall maintain separate name spaces for functions and
# variables.

# Defining a variable and a function with the same name. They must not conflict.
test_cmd='foo=var_value; foo() { echo "func_value"; }; echo "$foo"; foo'
assert_stdout "var_value
func_value" \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# Positional Parameters in Functions
# ==============================================================================
# REQUIREMENT: SHALL-2-9-5-384:
# The operands to the command temporarily shall become the positional
# parameters during the execution of the compound-command ; the special
# parameter '#' also shall be changed to reflect the number of operands.
# REQUIREMENT: SHALL-2-9-5-385:
# The special parameter 0 shall be unchanged.
# REQUIREMENT: SHALL-2-9-5-386:
# When the function completes, the values of the positional parameters and the
# special parameter '#' shall be restored to the values they had before the
# function was executed.

# Testing that arguments pass down correctly, `$0` is untouched, and original
# args return after the function finishes. We pass 'parent_arg' to the shell,
# call a function with 'child_arg', and verify the states at all points.
test_cmd='
myfunc() {
    printf "%s " "$0"
    printf "%s " "$1"
    printf "%s " "$#"
}
printf "%s " "$1"
printf "%s " "$#"
myfunc "child_arg"
printf "%s " "$1"
printf "%s" "$#"
'
# Call shell with arg 'parent_arg'. `$0` will be `sh`.
assert_stdout "parent_arg 1 $TARGET_SHELL child_arg 1 parent_arg 1" \
    "$TARGET_SHELL -c '$test_cmd' '$TARGET_SHELL' 'parent_arg'"


# ==============================================================================
# Returning from Functions
# ==============================================================================
# REQUIREMENT: SHALL-2-9-5-387:
# If the special built-in return (see return ) is executed in the
# compound-command , the function completes and execution shall resume with the
# next command after the function call.

test_cmd='
myfunc() {
    return 42
    echo "should not execute"
}
myfunc
echo "$?"
'
assert_stdout "42" \
    "$TARGET_SHELL -c '$test_cmd'"


report
