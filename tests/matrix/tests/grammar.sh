# Test: Shell Grammar and Lexical Conventions
# Target: tests/matrix/tests/grammar.sh
#
# Shell Grammar rules dictate exactly how the shell identifies constructs
# like loops, conditionals, and assignments. This suite validates these
# precise parsing rules.

. "$MATRIX_DIR/lib.sh"

# ==============================================================================
# Grammar Token Identification
# ==============================================================================
# REQUIREMENT: SHALL-2-10-1-396: If the string consists solely of digits and the
# delimiter character is one of '<' or '>' , the token identifier IO_NUMBER
# shall result.
# REQUIREMENT: SHALL-2-10-1-397: If the string contains at least three
# characters, begins with a <left-curly-bracket> ( '{' ) and ends with a <right-
# curly-bracket> ( '}' ), and the delimiter character is one of '<' or '>' , the
# token identifier IO_LOCATION may result; if the result is not IO_LOCATION ,
# the token identifier TOKEN shall result.

# We test that a digit immediately preceding a redirection is parsed as the
# IO_NUMBER token (file descriptor), not as a separate word/command.
test_cmd='echo content > tmp_grammar.txt; 0<tmp_grammar.txt'
assert_exit_code 0 \
    "$TARGET_SHELL -c '$test_cmd'"

# But if there's a space, it parses as the command `0` with standard
# redirection.
test_cmd='echo content > tmp_grammar.txt; 0 <tmp_grammar.txt'
assert_exit_code_non_zero \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# Assignment Parsing Rules
# ==============================================================================
# REQUIREMENT: SHALL-2-10-2-415: [Assignment preceding command name] [When the
# first word] If the TOKEN is exactly a reserved word, the token identifier for
# that reserved word shall result.
# REQUIREMENT: SHALL-2-10-2-416: Otherwise, 7b shall be applied.
# REQUIREMENT: SHALL-2-10-2-417: [Not the first word] If the TOKEN contains an
# unquoted (as determined while applying rule 4 from 2.3 Token Recognition )
# <equals-sign> character that is not part of an embedded parameter expansion,
# command substitution, or arithmetic expansion construct (as determined while
# applying rule 5 from 2.3 Token Recognition ): If the TOKEN begins with '=' ,
# then the token WORD shall be returned.
# REQUIREMENT: SHALL-2-10-2-418: If all the characters in the TOKEN preceding
# the first such <equals-sign> form a valid name (see XBD 3.216 Name ), the
# token ASSIGNMENT_WORD shall be returned.

# A valid assignment prefix correctly scopes to the command.
test_cmd='var=1 env | grep -q "^var=1$" && echo "assigned"'
assert_stdout 'assigned' \
    "$TARGET_SHELL -c '$test_cmd'"

# An invalid name cannot be an assignment, it evaluates as a command name
# and fails.
test_cmd='1invalid=true sh -c "echo executed"'
assert_exit_code_non_zero \
    "$TARGET_SHELL -c '$test_cmd'"

# A string starting with `=` is just a WORD (a command name).
test_cmd='=foo'
assert_exit_code_non_zero \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# "in" and "do" parsing in Loops
# ==============================================================================
# REQUIREMENT: SHALL-2-10-2-409: [ NAME in for ] When the TOKEN meets the
# requirements for a name (see XBD 3.216 Name ), the token identifier NAME shall
# result.
# REQUIREMENT: SHALL-2-10-2-404: Otherwise, the token WORD shall be returned.
# REQUIREMENT: SHALL-2-10-2-413: [ for only] When the TOKEN is exactly the
# reserved word in or do , the token identifier for in or do shall result,
# respectively.

# The name must be a valid identifier. If it is not, it should syntax error.
test_cmd='for 1invalid in a; do echo $1invalid; done'
assert_exit_code_non_zero \
    "$TARGET_SHELL -c '$test_cmd'"

# "in" is required as the third word if words are provided.
test_cmd='for i in a; do echo $i; done'
assert_stdout "a" \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# "case" statement parsing
# ==============================================================================
# REQUIREMENT: SHALL-2-10-2-407: [Case statement termination] When the TOKEN is
# exactly the reserved word esac , the token identifier for esac shall result.
# REQUIREMENT: SHALL-2-10-2-404: Otherwise, the token WORD shall be returned.
# REQUIREMENT: SHALL-2-10-2-411: [Third word of for and case ] [ case only] When
# the TOKEN is exactly the reserved word in , the token identifier for in shall
# result.

# Case statement needs `in` and `esac`.
test_cmd='
case "foo" in
    foo) echo "matched" ;;
esac
'
assert_stdout "matched" \
    "$TARGET_SHELL -c '$test_cmd'"


report
