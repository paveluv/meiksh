# Test: Quoting and Token Recognition
# Target: tests/matrix/tests/token_recognition.sh
#
# Welcome to the Quoting and Token Recognition suite! Here we explore how the
# shell interprets special characters, quoting mechanisms, and boundaries.
# According to POSIX, quoting is our shield against the shell's eagerness to
# expand and evaluate special symbols.

. "$MATRIX_DIR/lib.sh"


# ==============================================================================
# The Humble Backslash
# ==============================================================================
# REQUIREMENT: SHALL-2-2-001: The application shall quote the following
# characters if they are to represent themselves:...

# To represent special characters literally, they must be quoted. Here we quote
# a pipe, an ampersand, and a semicolon using a backslash.
assert_stdout '|&;' \
    "$TARGET_SHELL -c 'echo \|\\&\;'"

# REQUIREMENT: SHALL-2-2-1-002: A <backslash> that is not quoted shall preserve
# the literal value of the following character, with the exception of a
# <newline>.

# We test this by escaping an asterisk (which normally triggers filename
# generation) and a dollar sign (which normally triggers parameter expansion).
# They should emerge completely untouched!
assert_stdout 'a*b' \
    "$TARGET_SHELL -c 'echo a\\*b'"

assert_stdout '$foo' \
    "$TARGET_SHELL -c 'echo \\\$foo'"

# REQUIREMENT: SHALL-2-2-1-003: If a <newline> immediately follows the
# <backslash>, the shell shall interpret this as line continuation.
# REQUIREMENT: SHALL-2-2-1-004: The <backslash> and <newline> shall be removed
# before splitting the input into tokens.

# We put a backslash at the very end of the line, right before the newline. The
# shell must swallow both the backslash and the newline, stitching the two lines
# into a single command, making `ec\nho` execute as `echo`.
test_cmd='ec\
ho line continuation'
assert_stdout 'line continuation' \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# Single Quotes: The Absolute Literal
# ==============================================================================
# REQUIREMENT: SHALL-2-2-2-005: Enclosing characters in single-quotes ('') shall
# preserve the literal value of each character within the single-quotes.

# Inside single quotes, absolutely nothing is special. Even our mighty dollar
# sign and wildcards become mere characters. We pass an aggressively special
# string inside single quotes to prove the shell turns a blind eye to it all.
test_cmd="echo '\$foo *'"
assert_stdout '$foo *' \
    "$TARGET_SHELL -c \"\$test_cmd\""


# ==============================================================================
# Double Quotes: The Compromise
# ==============================================================================
# REQUIREMENT: SHALL-2-2-3-006: Enclosing characters in double-quotes ("") shall
# preserve the literal value of all characters within the double-quotes, with
# the exception of the characters backquote, <dollar-sign>, and <backslash>

# Double quotes stop word splitting and wildcard expansion, but leave the door
# open for parameter/command substitution and backslash escapes. Here, our
# asterisk remains literal, but our backslash must be doubled up to survive.
assert_stdout 'a*b' \
    "$TARGET_SHELL -c 'echo \"a*b\"'"

assert_stdout '\' \
    "$TARGET_SHELL -c 'echo \"\\\\\"'"

# REQUIREMENT: SHALL-2-2-3-007: The <dollar-sign> shall retain its special
# meaning introducing parameter expansion (see 2.6.2 Parameter Expansion), a
# form of command substitution (see 2.6.3 Command Substitution), and arithmetic
# expansion (see 2.6.4 Arithmetic Expansion), but shall not retain its special
# meaning introducing the dollar-single-quotes form of quoting.

# We test that inside double quotes, $foo expands, $(echo ...) executes,
# $((...)) evaluates, but $'...' is treated literally as $ and single quotes.
test_cmd="foo=bar; echo \"\$foo \$(echo sub) \$((2+2)) \$'literal'\""
assert_stdout "bar sub 4 \$'literal'" \
    "$TARGET_SHELL -c \"\$test_cmd\""

# REQUIREMENT: SHALL-2-2-3-008: The input characters within the quoted string
# that are also enclosed between "$(" and the matching ')' shall not be affected
# by the double-quotes, but rather shall define that command whose output
# replaces the "$(...)".

# We pass a command inside "$(...)" that uses unescaped double quotes. The
# inner double quotes must be treated normally for the inner command, proving
# the outer double quotes didn't affect them.
test_cmd='echo "$(echo "inner quotes")"'
assert_stdout 'inner quotes' \
    "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-2-2-3-009: The tokenizing rules in 2.3 Token Recognition
# shall be applied recursively to find the matching ')'.

# A deeply nested subshell with mismatched quotes inside it, proving the shell
# parses token boundaries recursively to find the correct `)` instead of the
# first one.
test_cmd='echo "$(echo "(recursive)")"'
assert_stdout '(recursive)' \
    "$TARGET_SHELL -c '$test_cmd'"


# REQUIREMENT: SHALL-2-2-3-014: The backquote shall retain its special meaning
# introducing the other form of command substitution.

# We test that inside double quotes, `echo ...` executes.
test_cmd="echo \"\`echo sub\`\""
assert_stdout 'sub' \
    "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-2-2-3-015: Outside of "$(...)" and "${...}" the <backslash>
# shall retain its special meaning as an escape character only when immediately
# followed by one of the following characters: $ ` \ <newline>

# If followed by an ordinary character like 'n', the backslash and 'n' should
# be printed literally inside double quotes.
test_cmd='printf "%s\n" "\n \$ \` \\"'
assert_stdout '\n $ ` \' \
    "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-2-2-3-016: When double-quotes are used to quote a parameter
# expansion, command substitution, or arithmetic expansion, the literal value of
# all characters within the result of the expansion shall be preserved.

# We test this by expanding a variable containing multiple spaces and asterisks
# inside double quotes, and verify they are preserved and not split/expanded.
test_cmd="foo='* * *'; echo \"\$foo\""
assert_stdout '* * *' \
    "$TARGET_SHELL -c \"\$test_cmd\""

# ==============================================================================
# Parameter Expansion Variations in Double Quotes
# ==============================================================================
# REQUIREMENT: SHALL-2-2-3-010: For the four varieties of parameter expansion
# that provide for substring processing (see 2.6.2 Parameter Expansion), within
# the string of characters from an enclosed "${" to the matching '}', the
# double-quotes within which the expansion occurs shall have no effect on the
# behavior of any ordinary, shell special, or pattern special characters...
#
# REQUIREMENT: SHALL-2-2-3-011: For parameter expansions other than the four
# varieties that provide for substring processing, within the string of
# characters from an enclosed "${" to the matching '}', the double-quotes within
# which the expansion occurs shall preserve the literal value of all
# characters...

# In substring expansions like `${foo#bar}`, the `bar` portion is NOT affected
# by the outer double quotes. We test this by using unquoted `*` inside the
# substring pattern, and verify it behaves as a pattern, while in a default
# value expansion `${foo:-*}`, the `*` remains a literal.
test_cmd='foo="a*b"; unset unset_var; echo "${foo#a*}" "${unset_var:-*}"'
assert_stdout '*b *' \
    "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-2-2-3-012: The backquote and <dollar-sign> characters shall
# follow the same rules as for characters in double-quotes described in 2.2.3.
# REQUIREMENT: SHALL-2-2-3-013: The <backslash> character shall follow the same
# rules as for characters in double-quotes described in 2.2.3 Double-Quotes.

# Within `${...}`, `\`, `$`, and `\`` still retain their double-quote rules.
test_cmd='unset foo; printf "%s\n" "${foo:-`echo default` \$ \n \\ }"'
assert_stdout 'default $ \n \ ' \
    "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-2-2-3-017: The application shall ensure that a double-quote
# that is not within "$(...)" nor within "${...}" is immediately preceded by a
# <backslash> in order to be included within double-quotes.

# Escaping a double quote inside double quotes correctly includes it.
assert_stdout '"' \
    "$TARGET_SHELL -c 'echo \"\\\"\"'"

# ==============================================================================
# Dollar-Single-Quotes: The C-Style Strings
# ==============================================================================
# REQUIREMENT: SHALL-2-2-4-018: A sequence of characters starting with a
# <dollar-sign> immediately followed by a single-quote ($') shall preserve the
# literal value of all characters within the single-quotes, with the exception
# of the <backslash> character.

# The $'...' quoting mechanism allows C-style escape sequences like \n, \t, etc.
test_cmd="echo \$'a\\nb'"
assert_stdout "a
b" \
    "$TARGET_SHELL -c \"\$test_cmd\""

# REQUIREMENT: SHALL-2-2-4-020: These <backslash>-escape sequences shall be
# processed (replaced with the bytes or characters they yield) before the
# token is processed for expansions or word splitting.

# REQUIREMENT: SHALL-2-2-4-019: In cases where a variable number of characters
# can be used to specify an escape sequence (\xXX and \uXXXX ...
#
# REQUIREMENT: SHALL-2-2-4-021: However, implementations shall not replace an
# unsupported character with bytes that do not form valid...

# We test that \x41 produces 'A', \x42 produces 'B', etc.
test_cmd="echo \$'\\x41\\x42'"
assert_stdout "AB" \
    "$TARGET_SHELL -c \"\$test_cmd\""

# REQUIREMENT: SHALL-2-2-4-022: If a <backslash>-escape sequence represents a
# single-quote character (for example \'), that sequence shall not terminate
# the dollar-single-quote processing.

# An escaped single quote inside $'...' must not end the string.
test_cmd="echo \$'quote: \\', done'"
assert_stdout "quote: ', done" \
    "$TARGET_SHELL -c \"\$test_cmd\""

# ==============================================================================
# The Newline Token Delimiter
# ==============================================================================
# REQUIREMENT: SHALL-2-3-010: If the current character is a <newline>, it shall
# delimit the current token.

# A newline isn't just whitespace; it's a hard boundary that ends commands and
# tokens. We'll feed the shell two distinct echo commands separated by a newline
# and verify it processed them sequentially.
assert_stdout 'ab' \
    "$TARGET_SHELL -c 'echo a
echo b' | tr -d '\n'"


report
