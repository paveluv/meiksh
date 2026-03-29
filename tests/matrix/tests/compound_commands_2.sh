# Test: Compound Commands (Part 2)
# Target: tests/matrix/tests/compound_commands_2.sh
#
# Additional POSIX Shell requirements for compound lists, case statements,
# function syntax error properties, and other complex structures.

. "$MATRIX_DIR/lib.sh"

# ==============================================================================
# Compound Lists and Exit Statuses
# ==============================================================================
# REQUIREMENT: SHALL-2-9-4-345:
# The exit status of a compound-list shall be the value that the special
# parameter '?' (see 2.5.2 Special Parameters ) would have immediately after
# execution of the compound-list .
# REQUIREMENT: SHALL-2-9-4-2-350:
# First, the list of words following in shall be expanded to generate a list of
# items.

test_cmd='
for i in "a b" c; do
    echo "$i"
done
'
assert_stdout "a b
c" \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# Case Statement Extensions
# ==============================================================================
# REQUIREMENT: SHALL-2-9-4-3-358:
# Each case statement clause, with the possible exception of the last, shall be
# terminated with either ";;" or ";&" .
# REQUIREMENT: SHALL-2-9-4-3-359:
# In order from the beginning to the end of the case statement, each pattern
# that labels a compound-list shall be subjected to tilde expansion, parameter
# expansion, command substitution, and arithmetic expansion, and the result of
# these expansions shall be compared against the expansion of word , according
# to the rules described in 2.14 Pattern Matching Notation (which also describes
# the effect of quoting parts of the pattern).
# REQUIREMENT: SHALL-2-9-4-3-362:
# If the case statement clause is terminated by ";&" , then the compound-list
# (if any) of each subsequent clause shall be executed, in order, until either a
# clause terminated by ";;" is reached and its compound-list (if any) executed
# or there are no further clauses in the case statement.

# Test ;; termination (SHALL-2-9-4-3-358)
test_cmd='
case "xyz" in
    abc) echo no ;;
    xyz) echo yes ;;
    *) echo default ;;
esac
'
assert_stdout "yes" \
    "$TARGET_SHELL -c '$test_cmd'"

# Test pattern expansion in case labels (SHALL-2-9-4-3-359)
test_cmd='
X=hello
case "hello" in
    $X) echo matched ;;
    *) echo nomatch ;;
esac
'
assert_stdout "matched" \
    "$TARGET_SHELL -c '$test_cmd'"

# Pattern with command substitution in case label
test_cmd='
case "3" in
    $(echo 3)) echo three ;;
    *) echo other ;;
esac
'
assert_stdout "three" \
    "$TARGET_SHELL -c '$test_cmd'"

# Pattern with arithmetic expansion in case label
test_cmd='
case "6" in
    $((2+4))) echo six ;;
    *) echo other ;;
esac
'
assert_stdout "six" \
    "$TARGET_SHELL -c '$test_cmd'"

# Test ;& fallthrough (SHALL-2-9-4-3-362) — POSIX.1-2024 feature
# Note: ;& was added in POSIX Issue 8 (2024). Shells that don't support it
# will fail this test, which is correct — they are non-conformant to Issue 8.
_out=$($TARGET_SHELL -c 'case a in a) echo first ;& b) echo second ;; c) echo third ;; esac' 2>/dev/null)
case "$_out" in
    "first
second")
        pass ;;
    "first")
        fail "case ;& fallthrough did not execute subsequent clause" ;;
    *)
        fail "case ;& test produced unexpected output: $(echo "$_out" | head -1)" ;;
esac

# ;& should fall through multiple clauses until ;;
_out=$($TARGET_SHELL -c 'case x in x) echo one ;& y) echo two ;& z) echo three ;; w) echo four ;; esac' 2>/dev/null)
case "$_out" in
    "one
two
three")
        pass ;;
    *)
        fail "case ;& multi-fallthrough expected one/two/three, got: $(echo "$_out" | tr '\n' '/')" ;;
esac

# ==============================================================================
# Syntax Spacing
# ==============================================================================
# REQUIREMENT: SHALL-2-9-4-1-347:
# A conforming application shall ensure that it separates the two leading '('
# characters with white space to prevent the shell from performing an arithmetic
# evaluation.

# `( ( echo hello ) )` must be accepted.
test_cmd='( ( echo hello ) )'
assert_stdout "hello" \
    "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-2-9-2-312:
# The format for a pipeline is: [ ! ] command1 [ | command2 ... ] If the
# pipeline begins with the reserved word ! and command1 is a subshell command,
# the application shall ensure that the ( operator at the beginning of command1
# is separated from the ! by one or more <blank> characters.

test_cmd='! ( false )'
assert_exit_code 0 \
    "$TARGET_SHELL -c '$test_cmd'"

report
