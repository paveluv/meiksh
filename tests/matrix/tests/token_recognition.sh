# Test: Quoting and Token Recognition
# Target: tests/matrix/tests/token_recognition.sh
#
# Welcome to the Quoting and Token Recognition suite! Here we explore how the
# shell interprets special characters, quoting mechanisms, and boundaries.
# According to POSIX, quoting is our shield against the shell's eagerness to
# expand and evaluate special symbols.

. "$MATRIX_DIR/lib.sh"


# ==============================================================================
# Token Recognition
# ==============================================================================
# REQUIREMENT: SHALL-2-3-023: The shell shall read its input in terms of lines....
# REQUIREMENT: SHALL-2-3-024: These lines shall be parsed using two major
# modes: ordinary token recognition and processing of here...
# REQUIREMENT: SHALL-2-3-025: When an io_here token has been recognized by the
# grammar (see 2.10 Shell Grammar), one or more of th...
# REQUIREMENT: SHALL-2-3-026: Any non-NEWLINE tokens (including more io_here
# tokens) that are recognized while searching for the n...
# REQUIREMENT: SHALL-2-3-027: If a saved token is an io_here token, the
# corresponding here-document shall start on the line immedi...
# REQUIREMENT: SHALL-2-3-028: When it is not processing an io_here, the shell
# shall break its input into tokens by applying the fi...
# REQUIREMENT: SHALL-2-3-029: At the start of input or after a previous token
# has just been delimited, the first or next token, re...
# REQUIREMENT: SHALL-2-3-030: Once a token has started, zero or more characters
# from the input shall be appended to the token unti...
# REQUIREMENT: SHALL-2-3-031: When both the start and end of a token have been
# delimited, the characters forming the token shall b...
# REQUIREMENT: SHALL-2-3-032: If a rule below indicates that a token is
# delimited, and no characters have been included in the tok...
# REQUIREMENT: SHALL-2-3-033: If the end of input is recognized, the current
# token (if any) shall be delimited....
# REQUIREMENT: SHALL-2-3-036: If the current character is an unquoted
# <backslash>, single-quote, or double-quote or is the first c...
# REQUIREMENT: SHALL-2-3-037: During token recognition no substitutions shall
# be actually performed, and the result token shall co...
# REQUIREMENT: SHALL-2-3-038: The token shall not be delimited by the end of
# the quoted field....
# REQUIREMENT: SHALL-2-3-039: If the current character is an unquoted '$' or '`',
# the shell shall identify the start of any candid...
# REQUIREMENT: SHALL-2-3-040: The shell shall read sufficient input to determine
# the end of the unit to be expanded (as explained ...
# REQUIREMENT: SHALL-2-3-041: While processing the characters, if instances of
# expansions or quoting are found nested within the s...
# REQUIREMENT: SHALL-2-3-042: For "$(" and '`' only, if instances of io_here
# tokens are found nested within the substitution, they...
# REQUIREMENT: SHALL-2-3-043: The characters found from the beginning of the
# substitution to its end, allowing for any recursion n...
# REQUIREMENT: SHALL-2-3-044: The token shall not be delimited by the end of
# the substitution....
# REQUIREMENT: SHALL-2-3-050: In situations where the shell parses its input as
# a program, once a complete_command has been recogn...
# REQUIREMENT: SHALL-2-3-1-118: If it is not indicated within a ${...}
# parameter expansion, the shell shall treat it as a syntax err...
# REQUIREMENT: SHALL-2-3-1-119: An unquoted <backslash> shall retain its
# absolute literal meaning when followed by a <newline>...
# REQUIREMENT: SHALL-DESCRIPTION-001: The sh utility is a command language
# interpreter that shall execute commands read from a command lin...
# REQUIREMENT: SHALL-DESCRIPTION-002: The application shall ensure that the
# commands to be executed are expressed in the language describe...

test_cmd='echo a\
b'
assert_stdout "ab" \
    "$TARGET_SHELL -c '$test_cmd'"

# Bad expansion parameter error
test_cmd='echo ${/}'
assert_exit_code_non_zero \
    "$TARGET_SHELL -c '$test_cmd' 2>/dev/null"

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
