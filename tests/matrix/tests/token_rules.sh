# Test: Token Recognition Rules
# Target: tests/matrix/tests/token_rules.sh
#
# POSIX token recognition consists of 10 primary rules plus alias substitution.
# This suite specifically tests boundaries, operators, blank separation, and
# comment recognition.

. "$MATRIX_DIR/lib.sh"

# ==============================================================================
# Operators vs Words
# ==============================================================================
# REQUIREMENT: SHALL-2-3-045: If the current character is not quoted and can be
# used as the first character of a new operator, the current token...
# REQUIREMENT: SHALL-2-3-046: The current character shall be used as the
# beginning of the next (operator) token.

# An unquoted `>` is a control operator. Even without spaces, `echo a>b` splits
# into `echo`, `a`, `>`, and `b`.
test_cmd='echo a>tmp_token.txt; cat tmp_token.txt'
assert_stdout 'a' \
    "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-2-3-034: If the previous character was used as part of an
# operator and the current character is not quoted and can be used with...
# REQUIREMENT: SHALL-2-3-035: If the previous character was used as part of an
# operator and the current character cannot be used...

# `>>` forms a single operator. `> >` forms two. `>|` forms one operator.
test_cmd='echo a >tmp_token.txt; echo b >>tmp_token.txt; cat tmp_token.txt'
assert_stdout "a
b" \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# Blanks and Words
# ==============================================================================
# REQUIREMENT: SHALL-2-3-047: If the current character is an unquoted <blank>,
# any token containing the previous character is delimited...
# REQUIREMENT: SHALL-2-3-048: If the previous character was part of a word, the
# current character shall be appended to that word.

# Multiple blanks between words simply delimit them.
test_cmd='echo a      b'
assert_stdout 'a b' \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# Comments
# ==============================================================================
# REQUIREMENT: SHALL-2-3-049: If the current character is a '#', it and all
# subsequent characters up to, but excluding, the next <newline>...

# Comments must be ignored up to the newline.
test_cmd='echo a # this is a comment
echo b'
assert_stdout "a
b" \
    "$TARGET_SHELL -c '$test_cmd'"

# If `#` is quoted, it is not a comment.
test_cmd='echo "a # not a comment"'
assert_stdout 'a # not a comment' \
    "$TARGET_SHELL -c '$test_cmd'"

# If `#` is in the middle of a word, it is not a comment.
test_cmd='echo a#b'
assert_stdout 'a#b' \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# Alias Substitution
# ==============================================================================
# REQUIREMENT: SHALL-2-3-1-051: After a token has been categorized as type
# TOKEN... if it is a valid name... the token shall be replaced.
# REQUIREMENT: SHALL-2-3-1-052: When a TOKEN is subject to alias substitution,
# the value of the alias shall be processed as if it had...
# REQUIREMENT: SHALL-2-3-1-053: If the value of the alias replacing the word
# ends in a <blank>, the shell shall check the next command word for alias...
# REQUIREMENT: SHALL-2-3-1-054: Changes to aliases shall not take effect out of
# order....
# REQUIREMENT: SHALL-2-3-1-055: When used as specified by this volume of
# POSIX.1-2024, alias definitions shall not be inherited by subshells.

# NOTE: Aliases are only expanded if `expand_aliases` is enabled or in
# interactive shells. We'll test it if the shell supports it or run it in
# interactive mode using PTY.
# However, POSIX says aliases are processed for interactive shells or when
# explicitly enabled.

interactive_script=$(cat << 'EOF'
sleep 0.5
echo 'alias foo="echo aliased"'
sleep 0.5
echo 'foo'
sleep 0.5
echo 'exit'
EOF
)

cmd="( $interactive_script ) | \"$MATRIX_DIR/pty\" $TARGET_SHELL -i"
actual=$(eval "$cmd" 2>&1)

case "$actual" in
    *"aliased"*)
        pass
        ;;
    *)
        fail "Expected alias substitution to print 'aliased', got: $actual"
        ;;
esac

# Test alias with trailing space allowing subsequent word to be aliased.
interactive_script=$(cat << 'EOF'
sleep 0.5
echo 'alias a1="echo "'
echo 'alias a2="chained"'
sleep 0.5
echo 'a1 a2'
sleep 0.5
echo 'exit'
EOF
)

cmd="( $interactive_script ) | \"$MATRIX_DIR/pty\" $TARGET_SHELL -i"
actual=$(eval "$cmd" 2>&1)

case "$actual" in
    *"chained"*)
        pass
        ;;
    *)
        fail "Expected chained alias substitution to print 'chained', got: $actual"
        ;;
esac

report
