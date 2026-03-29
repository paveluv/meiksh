# Test: Special Built-ins (break, continue, colon, dot, eval)
# Target: tests/matrix/tests/builtins_1.sh
#
# POSIX Shell mandates several special built-ins. Here we verify the control
# flow modifiers (break, continue), the null utility (:), the execution
# string builder (eval), and the sourcing utility (dot / .).

. "$MATRIX_DIR/lib.sh"

# ==============================================================================
# The 'break' Utility
# ==============================================================================
# REQUIREMENT: SHALL-DESCRIPTION-512:
# If n is specified, the break utility shall exit from the n th enclosing for ,
# while , or until loop.
# REQUIREMENT: SHALL-DESCRIPTION-515:
# The application shall ensure that the value of n is a positive decimal
# integer.
# REQUIREMENT: SHALL-DESCRIPTION-517:
# A loop shall enclose a break or continue command if the loop lexically
# encloses the command.
# REQUIREMENT: SHALL-DESCRIPTION-513:
# If n is not specified, break shall behave as if n was specified as 1.
# REQUIREMENT: SHALL-DESCRIPTION-514:
# Execution shall continue with the command immediately following the exited
# loop.
# REQUIREMENT: SHALL-DESCRIPTION-516:
# If n is greater than the number of enclosing loops, the outermost enclosing
# loop shall be exited.

# Test `break` without n (defaults to 1).
test_cmd='for i in 1 2 3; do echo $i; break; echo "no"; done; echo "done"'
assert_stdout "1
done" \
    "$TARGET_SHELL -c '$test_cmd'"

# Test `break 2` (exits 2 loops).
test_cmd='for i in a b; do for j in 1 2; do echo "$i$j"; break 2; done; done; echo "done"'
assert_stdout "a1
done" \
    "$TARGET_SHELL -c '$test_cmd'"

# Test `break N` where N > enclosing loops.
test_cmd='for i in a b; do echo $i; break 5; done; echo "done"'
assert_stdout "a
done" \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# The 'continue' Utility
# ==============================================================================
# REQUIREMENT: SHALL-DESCRIPTION-522:
# If n is specified, the continue utility shall return to the top of the n th
# enclosing for , while , or until loop.
# REQUIREMENT: SHALL-DESCRIPTION-523:
# If n is not specified, continue shall behave as if n was specified as 1.
# REQUIREMENT: SHALL-DESCRIPTION-515:
# The application shall ensure that the value of n is a positive decimal
# integer.
# REQUIREMENT: SHALL-DESCRIPTION-525:
# If n is greater than the number of enclosing loops, the outermost enclosing
# loop shall be used.

# Test `continue` without n (defaults to 1).
test_cmd='for i in 1 2; do echo $i; continue; echo "no"; done; echo "done"'
assert_stdout "1
2
done" \
    "$TARGET_SHELL -c '$test_cmd'"

# Test `continue 2` (returns to top of 2nd loop).
test_cmd='for i in a b; do echo "outer $i"; for j in 1 2; do echo "inner $j"; continue 2; done; done; echo "done"'
assert_stdout "outer a
inner 1
outer b
inner 1
done" \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# The '.' Utility and Errors
# ==============================================================================
# REQUIREMENT: SHALL-EXIT-STATUS-533:
# If no readable file was found or if the commands in the file could not be
# parsed, and the shell is interactive (and therefore does not abort; see 2.8.1
# Consequences of Shell Errors ), the exit status shall be non-zero.

# ==============================================================================
# The ':' (Colon / Null) Utility
# ==============================================================================
# REQUIREMENT: SHALL-DESCRIPTION-519:
# This utility shall do nothing except return a 0 exit status.
# REQUIREMENT: SHALL-OPTIONS-520:
# This utility shall not recognize the "--" argument in the manner specified by
# Guideline 10 of XBD 12.2 Utility Syntax Guidelines .
# REQUIREMENT: SHALL-OPTIONS-521:
# Implementations shall not support any options.

test_cmd=':'
assert_exit_code 0 \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# The 'eval' Utility
# ==============================================================================
# REQUIREMENT: SHALL-DESCRIPTION-534:
# The eval utility shall construct a command string by concatenating argument s
# together, separating each with a <space> character.
# REQUIREMENT: SHALL-DESCRIPTION-535:
# The constructed command string shall be tokenized (see 2.3 Token Recognition
# ), parsed (see 2.10 Shell Grammar ), and executed by the shell in the current
# environment.

test_cmd='foo="bar"; eval echo "\$foo" "and" "baz"'
assert_stdout "bar and baz" \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# The '.' (Dot) Utility
# ==============================================================================
# REQUIREMENT: SHALL-DESCRIPTION-526:
# The meaning of "enclosing" shall be as specified in the description of the
# break utility.
# REQUIREMENT: SHALL-DESCRIPTION-528:
# The shell shall tokenize (see 2.3 Token Recognition ) the contents of the
# file , parse the tokens (see 2.10 Shell Grammar ), and execute the resulting
# commands in the current environment.
# REQUIREMENT: SHALL-DESCRIPTION-529:
# If file does not contain a <slash>, the shell shall use the search path
# specified by PATH to find the directory containing file .
# REQUIREMENT: SHALL-DESCRIPTION-530:
# If no readable file is found, a non-interactive shell shall abort; an
# interactive shell shall write a diagnostic message to standard error.
# REQUIREMENT: SHALL-DESCRIPTION-531:
# The dot special built-in shall support XBD 12.2 Utility Syntax Guidelines ,
# except for Guidelines 1 and 2.

# Test dot sourcing a file in the current directory (using explicit path).
echo 'export SOURCED_VAR=hello' > tmp_source.sh
test_cmd='. ./tmp_source.sh; echo "$SOURCED_VAR"'
assert_stdout "hello" \
    "$TARGET_SHELL -c '$test_cmd'"

# Test dot sourcing via PATH resolution.
mkdir -p tmp_bin
echo 'export SOURCED_VAR=path_resolved' > tmp_bin/tmp_source.sh
test_cmd='PATH="$PWD/tmp_bin:$PATH" . tmp_source.sh; echo "$SOURCED_VAR"'
assert_stdout "path_resolved" \
    "$TARGET_SHELL -c '$test_cmd'"


report
