# Test: Pipelines and Lists
# Target: tests/matrix/tests/pipelines_and_lists.sh
#
# POSIX Shells are the glue of the Unix ecosystem. This test suite verifies
# the shell's ability to chain commands together using pipes (|), logical
# AND (&&), logical OR (||), and asynchronous execution (&).

. "$MATRIX_DIR/lib.sh"


# ==============================================================================
# Pipeline Execution
# ==============================================================================
# REQUIREMENT: SHALL-2-9-265: There shall be no limit on the size of any shell
# command other than that imposed by the underlying system...
# REQUIREMENT: SHALL-2-9-2-311: For each command but the last, the shell shall
# connect the standard output of the command to the standard input...
# REQUIREMENT: SHALL-2-9-2-313: The standard output of command1 shall be
# connected to the standard input of command2.
# REQUIREMENT: SHALL-2-9-2-315: If the pipeline is not in the background...
# the shell shall wait for the last command specified in the pipeline...
#
# REQUIREMENT: SHALL-2-9-2-314: The standard input, standard output, or both of
# a command shall be considered to be assigned by the pipeline before any
# redirection specified by redirection operators that are part of the command.

# The classic pipe: stdout from echo flows into stdin of tr.
test_cmd='echo "hello pipe" | tr "p" "t"'
assert_stdout "hello tite" \
    "$TARGET_SHELL -c '$test_cmd'"

# Testing that pipeline assignments happen BEFORE redirections:
# `echo hello | cat > file.txt` means cat's stdout is pipe by default, but the `>`
# overrides it.
test_cmd='echo "pipeline test" | cat > tmp_pipe.txt; cat tmp_pipe.txt'
assert_stdout "pipeline test" \
    "$TARGET_SHELL -c '$test_cmd'"

# Testing a pipeline with multiple stages.
test_cmd='echo "a" | sed "s/a/b/" | sed "s/b/c/"'
assert_stdout "c" \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# AND-OR Lists
# ==============================================================================
# REQUIREMENT: SHALL-2-9-3-318: The operators "&&" and "||" shall have equal
# precedence and shall be evaluated with left associativity.

# Testing left associativity:
# `false && echo foo || echo bar` -> (false && echo foo) fails, so it evaluates
# `echo bar`.
test_cmd='false && echo foo || echo bar'
assert_stdout "bar" \
    "$TARGET_SHELL -c '$test_cmd'"

# `true || echo foo && echo bar` -> (true || echo foo) succeeds (does not run foo),
# then it evaluates `echo bar`.
test_cmd='true || echo foo && echo bar'
assert_stdout "bar" \
    "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-2-9-3-3-337: First command1 shall be executed.
# REQUIREMENT: SHALL-2-9-3-3-338: If its exit status is zero, command2 shall be
# executed, and so on...

test_cmd='true && echo "and success"'
assert_stdout "and success" \
    "$TARGET_SHELL -c '$test_cmd'"

# The second command must not execute if the first returns non-zero.
test_cmd='false && echo "should not print"'
assert_stdout "" \
    "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-2-9-3-4-341: First, command1 shall be executed.
# REQUIREMENT: SHALL-2-9-3-4-342: If its exit status is non-zero, command2 shall
# be executed, and so on...

test_cmd='false || echo "or success"'
assert_stdout "or success" \
    "$TARGET_SHELL -c '$test_cmd'"

# The second command must not execute if the first returns zero.
test_cmd='true || echo "should not print"'
assert_stdout "" \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# Pipeline Exit Statuses
# ==============================================================================
# REQUIREMENT: SHALL-Exit Status-316: The exit status of a pipeline shall
# depend on whether or not the pipefail option (see set) is enabled...
# REQUIREMENT: SHALL-Exit Status-317: The shell shall use the pipefail setting
# at the time it begins execution of the pipeline...

test_cmd='false | true'
assert_exit_code 0 \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# Sequential Lists
# ==============================================================================
# REQUIREMENT: SHALL-Exit Status-316: The exit status of a pipeline shall
# depend on whether or not the pipefail option (see set) is enabled...
# REQUIREMENT: SHALL-Exit Status-317: The shell shall use the pipefail setting
# at the time it begins execution of the pipeline...

test_cmd='false | true'
assert_exit_code 0 \
    "$TARGET_SHELL -c '$test_cmd'"
# ==============================================================================
# REQUIREMENT: SHALL-2-9-3-2-331: AND-OR lists that are separated by a
# <semicolon> (';') shall be executed sequentially.
# REQUIREMENT: SHALL-2-9-3-2-332: The format for executing AND-OR lists
# sequentially shall be: AND-OR list [; AND-OR list]...
# REQUIREMENT: SHALL-2-9-3-2-333: Each AND-OR list shall be expanded and
# executed in the order specified.
# REQUIREMENT: SHALL-2-9-3-2-334: If job control is enabled, the AND-OR lists
# shall form all or part of a foreground job...
# REQUIREMENT: SHALL-Exit Status-335: The exit status of a sequential AND-OR
# list shall be the exit status of the last pipeline in the AND...

test_cmd='echo a; echo b; false'
assert_exit_code 1 \
    "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-2-9-3-319: false && echo foo || echo bar true || echo foo
# && echo bar A ';' separator or a ';' or <newline> terminator shall not be...

test_cmd='false && echo foo || echo bar; true || echo foo && echo bar'
assert_stdout "bar
bar" \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# AND Lists
# ==============================================================================
# REQUIREMENT: SHALL-2-9-3-3-336: The format shall be: command1 && command2
# REQUIREMENT: SHALL-Exit Status-339: The exit status of an AND list shall be
# the exit status of the last command that is executed in the...

test_cmd='true && false && echo no'
assert_exit_code 1 \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# OR Lists
# ==============================================================================
# REQUIREMENT: SHALL-2-9-3-4-340: The format shall be: command1 || command2
# REQUIREMENT: SHALL-Exit Status-343: The exit status of an OR list shall be
# the exit status of the last command that is executed in the...

test_cmd='false || false || true'
assert_exit_code 0 \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# Exit Status Tracking
# ==============================================================================
# REQUIREMENT: SHALL-2-9-264: Unless otherwise stated, the exit status of a
# command shall be that of the last simple command executed by the command.

# The exit status of the list `true; false` should be 1.
test_cmd='true; false'
assert_exit_code_non_zero \
    "$TARGET_SHELL -c '$test_cmd'"

# The exit status of the list `false; true` should be 0.
test_cmd='false; true'
assert_exit_code 0 \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# Async AND-OR lists
# ==============================================================================
# REQUIREMENT: SHALL-Exit Status-330: The exit status of an asynchronous AND-OR
# list shall be zero...

test_cmd='
false &
wait $!
exit 0
'
assert_exit_code 0 \
    "$TARGET_SHELL -c '$test_cmd'"

report
