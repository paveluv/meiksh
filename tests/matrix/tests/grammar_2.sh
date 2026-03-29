# Test: Shell Grammar and Lexical Conventions (Part 2)
# Target: tests/matrix/tests/grammar_2.sh
#
# POSIX Shell grammar precisely dictates how tokens are classified and
# interpreted based on their syntactic context.

. "$MATRIX_DIR/lib.sh"

# ==============================================================================
# Token Classification
# ==============================================================================
# REQUIREMENT: SHALL-2-10-390:
# This formal syntax shall take precedence over the preceding text syntax
# description.
# REQUIREMENT: SHALL-2-10-1-391:
# The input language to the shell shall be first recognized at the character
# level.
# REQUIREMENT: SHALL-2-10-1-392:
# The resulting tokens shall be classified by their immediate context according
# to the following rules (applied in order).
# REQUIREMENT: SHALL-2-10-1-393:
# These rules shall be used to determine what a "token" is that is subject to
# parsing at the token level.
# REQUIREMENT: SHALL-2-10-1-394:
# The rules for token recognition in 2.3 Token Recognition shall apply.
# REQUIREMENT: SHALL-2-10-1-395:
# If the token is an operator, the token identifier for that operator shall
# result.
# REQUIREMENT: SHALL-2-10-1-398:
# Otherwise, the token identifier TOKEN shall result.
# REQUIREMENT: SHALL-2-10-1-399:
# When a TOKEN is seen where one of those
# annotated productions could be used...
# REQUIREMENT: SHALL-2-10-1-400:
# The reduction shall then proceed based upon the token identifier type yielded
# by the rule applied.
# REQUIREMENT: SHALL-2-10-1-401:
# When more than one rule applies, the highest numbered rule shall apply (which
# in turn may refer to another rule). (Note that except in rule 7, the presence
# of an '=' in the token has no effect.)
# REQUIREMENT: SHALL-2-10-1-402:
# The WORD tokens shall have the word expansion rules applied to them
# immediately before the associated command is executed, not at the time the
# command is parsed.

test_cmd='var="value"; echo "$var"'
assert_stdout "value" \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# Command Name and Redirection Token Contexts
# ==============================================================================
# REQUIREMENT: SHALL-2-10-2-403:
# [Command Name] When the TOKEN is exactly a reserved word, the token
# identifier for that reserved word shall result.
# REQUIREMENT: SHALL-2-10-2-404:
# Otherwise, the token WORD shall be returned.
# REQUIREMENT: SHALL-2-10-2-405:
# [Redirection to or from filename] The expansions specified in 2.7 Redirection
# shall occur.
# REQUIREMENT: SHALL-2-10-2-406:
# [Redirection from here-document] Quote removal shall be applied to the word
# to determine the delimiter that is used to find the end of the here-document
# that begins after the next <newline>.

test_cmd='cat << "EOF"
$var
EOF'
assert_stdout "\$var" \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# "in" and "do" parsing fallbacks
# ==============================================================================
# REQUIREMENT: SHALL-2-10-2-404:
# Otherwise, the token WORD shall be returned.
# REQUIREMENT: SHALL-2-10-2-404:
# Otherwise, the token WORD shall be returned.
# REQUIREMENT: SHALL-2-10-2-404:
# Otherwise, the token WORD shall be returned.
# REQUIREMENT: SHALL-2-10-2-420:
# If a returned ASSIGNMENT_WORD token begins with a valid name, assignment of
# the value after the first <equals-sign> to the name shall occur as specified
# in 2.9.1 Simple Commands .

test_cmd='var=123 env | grep -q "^var=123" && echo "assignment"'
assert_stdout "assignment" \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# Function Definitions
# ==============================================================================
# REQUIREMENT: SHALL-2-10-2-421:
# [ NAME in function] When the TOKEN is exactly a reserved word, the token
# identifier for that reserved word shall result.
# REQUIREMENT: SHALL-2-10-2-422:
# Otherwise, when the TOKEN meets the requirements for a name, the token
# identifier NAME shall result.
# REQUIREMENT: SHALL-2-10-2-423:
# [Body of function] Word expansion and assignment shall never occur, even when
# required by the rules above, when this rule is being parsed.
# REQUIREMENT: SHALL-2-10-2-424:
# Each TOKEN that might either be expanded or have assignment applied to it
# shall instead be returned as a single WORD consisting only of characters that
# are exactly the token described in 2.3 Token Recognition .

test_cmd='
myfunc() {
    local_var="x"
    echo "$local_var"
}
myfunc
'
assert_stdout "x" \
    "$TARGET_SHELL -c '$test_cmd'"

report
