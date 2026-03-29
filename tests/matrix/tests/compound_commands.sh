# Test: Compound Commands (Loops, Conditionals, Grouping)
# Target: tests/matrix/tests/compound_commands.sh
#
# POSIX Shells support complex flow control via compound commands: loops
# (for, while, until), conditionals (if, case), and execution groups (subshells,
# brace groups). Here we verify their behavior and exit statuses.

. "$MATRIX_DIR/lib.sh"


# ==============================================================================
# The 'for' Loop
# ==============================================================================
# REQUIREMENT: SHALL-2-9-4-2-349:
# The for loop shall execute a sequence of commands for each member in a list
# of items .
# REQUIREMENT: SHALL-2-9-4-2-351:
# Then, the variable name shall be set to each item, in turn, and the
# compound-list executed each time.

# We test iterating over a set of items.
test_cmd='for i in a b c; do printf "%s " "$i"; done'
assert_stdout "a b c " \
    "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-2-9-4-2-352:
# If no items result from the expansion, the compound-list shall not be
# executed.
# REQUIREMENT: SHALL-2-9-4-2-1-353:
# If there is at least one item in the list of items, the exit status of a for
# command shall be the exit status of the last compound-list executed.
# REQUIREMENT: SHALL-2-9-4-2-1-354:
# If there are no items, the exit status shall be zero.

test_cmd='
for i in 1 2; do
    false
done
exit $?
'
assert_exit_code 1 \
    "$TARGET_SHELL -c '$test_cmd'"

# An empty list must execute 0 times. We set `x=1` and `x=2` inside the loop,
# but it should remain 1.
test_cmd='x=1; for i in; do x=2; done; echo "$x"'
assert_stdout "1" \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# The 'case' Construct
# ==============================================================================
# REQUIREMENT: SHALL-2-9-4-3-355:
# The conditional construct case shall execute the compound-list corresponding
# to the first pattern (see 2.14 Pattern Matching Notation ), if any are
# present, that is matched by the string resulting from the tilde expansion,
# parameter expansion, command substitution, arithmetic expansion, and quote
# removal of the given word.
# REQUIREMENT: SHALL-2-9-4-3-356:
# The reserved word in shall denote the beginning of the patterns to be
# matched.
# REQUIREMENT: SHALL-2-9-4-3-357:
# Multiple patterns with the same compound-list shall be delimited by the '|'
# symbol.
# REQUIREMENT: SHALL-2-9-4-3-360:
# After the first match, no more patterns in the case statement shall be
# expanded, and the compound-list of the matching clause shall be executed.
# REQUIREMENT: SHALL-2-9-4-3-1-363:
# The exit status of case shall be zero if no patterns are matched.
# REQUIREMENT: SHALL-2-9-4-3-1-364:
# Otherwise, the exit status shall be the exit status of the compound-list of
# the last clause to be executed.

test_cmd='
case "xyz" in
    abc) false ;;
    xyz) false ;;
esac
exit $?
'
assert_exit_code 1 \
    "$TARGET_SHELL -c '$test_cmd'"

test_cmd='
case "xyz" in
    abc) false ;;
esac
exit $?
'
assert_exit_code 0 \
    "$TARGET_SHELL -c '$test_cmd'"
# REQUIREMENT: SHALL-2-9-4-3-361:
# If the case statement clause is terminated by ";;" , no further clauses shall
# be examined.

# Testing matching with | and early exit via ;;.
test_cmd='
case "apple" in
    banana|orange) echo "no" ;;
    apple|pear) echo "yes" ;;
    *) echo "default" ;;
esac'
assert_stdout "yes" \
    "$TARGET_SHELL -c '$test_cmd'"

# Testing the default `*` match.
test_cmd='
case "grape" in
    apple) echo "no" ;;
    *) echo "default" ;;
esac'
assert_stdout "default" \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# The 'if' Construct
# ==============================================================================
# REQUIREMENT: SHALL-2-9-4-4-365:
# The if command shall execute a compound-list and use its exit status to
# determine whether to execute another compound-list .
# REQUIREMENT: SHALL-2-9-4-4-366:
# The format for the if construct is as follows: if compound-list then
# compound-list [ elif compound-list then compound-list ] ... [ else
# compound-list ] fi The if compound-list shall be executed; if its exit status
# is zero, the then compound-list shall be executed and the command shall
# complete.
# REQUIREMENT: SHALL-2-9-4-4-367:
# Otherwise, each elif compound-list shall be executed, in turn, and if its
# exit status is zero, the then compound-list shall be executed and the command
# shall complete.
# REQUIREMENT: SHALL-2-9-4-4-368:
# Otherwise, the else compound-list shall be executed.
# REQUIREMENT: SHALL-2-9-4-4-1-369:
# The exit status of the if command shall be the exit status of the then or
# else compound-list that was executed, or zero, if none was executed.

test_cmd='
if true; then
    false
fi
exit $?
'
assert_exit_code 1 \
    "$TARGET_SHELL -c '$test_cmd'"

# Test true `if`.
test_cmd='if true; then echo "if"; elif false; then echo "elif"; else echo "else"; fi'
assert_stdout "if" \
    "$TARGET_SHELL -c '$test_cmd'"

# Test true `elif`.
test_cmd='if false; then echo "if"; elif true; then echo "elif"; else echo "else"; fi'
assert_stdout "elif" \
    "$TARGET_SHELL -c '$test_cmd'"

# Test true `else`.
test_cmd='if false; then echo "if"; elif false; then echo "elif"; else echo "else"; fi'
assert_stdout "else" \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# The 'while' Loop
# ==============================================================================
# REQUIREMENT: SHALL-2-9-4-5-370:
# The while loop shall continuously execute one compound-list as long as
# another compound-list has a zero exit status.
# REQUIREMENT: SHALL-2-9-4-5-371:
# The format of the while loop is as follows: while compound-list-1 do
# compound-list-2 done The compound-list-1 shall be executed, and if it has a
# non-zero exit status, the while command shall complete.
# REQUIREMENT: SHALL-2-9-4-5-372:
# Otherwise, the compound-list-2 shall be executed, and the process shall
# repeat.
# REQUIREMENT: SHALL-2-9-4-5-1-373:
# The exit status of the while loop shall be the exit status of the last
# compound-list-2 executed, or zero if none was executed.

test_cmd='
while false; do
    true
done
exit $?
'
assert_exit_code 0 \
    "$TARGET_SHELL -c '$test_cmd'"

test_cmd='
counter=0
while [ "$counter" -lt 1 ]; do
    counter=$((counter + 1))
    false
done
exit $?
'
assert_exit_code 1 \
    "$TARGET_SHELL -c '$test_cmd'"

test_cmd='x=0; while [ $x -lt 3 ]; do x=$((x+1)); printf "%s " "$x"; done'
assert_stdout "1 2 3 " \
    "$TARGET_SHELL -c '$test_cmd'"

# Loop that immediately exits.
test_cmd='x=0; while false; do x=$((x+1)); done; echo "$x"'
assert_stdout "0" \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# The 'until' Loop
# ==============================================================================
# REQUIREMENT: SHALL-2-9-4-6-374:
# The until loop shall continuously execute one compound-list as long as
# another compound-list has a non-zero exit status.
# REQUIREMENT: SHALL-2-9-4-6-375:
# The format of the until loop is as follows: until compound-list-1 do
# compound-list-2 done The compound-list-1 shall be executed, and if it has a
# zero exit status, the until command completes.
# REQUIREMENT: SHALL-2-9-4-6-376:
# Otherwise, the compound-list-2 shall be executed, and the process repeats.
# REQUIREMENT: SHALL-2-9-4-6-1-377:
# The exit status of the until loop shall be the exit status of the last
# compound-list-2 executed, or zero if none was executed.

test_cmd='x=0; until [ $x -eq 3 ]; do x=$((x+1)); printf "%s " "$x"; done'
assert_stdout "1 2 3 " \
    "$TARGET_SHELL -c '$test_cmd'"

# Loop that immediately exits.
test_cmd='x=0; until true; do x=$((x+1)); done; echo "$x"'
assert_stdout "0" \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# Grouping Commands (Subshells)
# ==============================================================================
# REQUIREMENT: SHALL-2-9-4-1-346:
# Variable assignments and built-in commands that affect the environment shall
# not remain in effect after the list finishes.

# A subshell modifies variables locally, leaving the parent intact.
test_cmd='FOO=parent; (FOO=child); echo "$FOO"'
assert_stdout "parent" \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# Redirections on Compound Commands
# ==============================================================================
# REQUIREMENT: SHALL-2-9-4-344:
# Each redirection shall apply to all the commands within the compound command
# that do not explicitly override that redirection.

test_cmd='{ echo a; echo b; } > tmp_group.txt; cat tmp_group.txt'
assert_stdout "a
b" \
    "$TARGET_SHELL -c '$test_cmd'"


report
