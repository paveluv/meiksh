# Test: Pattern Matching Notation
# Target: tests/matrix/tests/pattern_matching.sh
#
# Pattern matching is the foundation of filename expansion (globbing) and `case`
# statement evaluation. Here we verify that `*`, `?`, and `[...]` behave
# exactly as specified by POSIX, including strict rules about slashes and
# hidden files (dotfiles).

. "$MATRIX_DIR/lib.sh"

# ==============================================================================
# Single Character Matching
# ==============================================================================
# REQUIREMENT: SHALL-2-14-1-486: ?A <question-mark> is a pattern that shall
# match any character.
# REQUIREMENT: SHALL-2-14-1-481: An ordinary character is a pattern that shall
# match itself.
# REQUIREMENT: SHALL-2-14-1-476: The following patterns shall match a single
# character: ordinary characters, special pattern characters...
# REQUIREMENT: SHALL-2-14-1-482: Matching shall be based on the bit pattern
# used for encoding the character, not on the graphic repre...
# REQUIREMENT: SHALL-2-14-1-485: When unquoted, unescaped, and not inside a
# bracket expression, the following three characters shall...

# Using a `case` statement to test `?` and ordinary characters.
test_cmd='
case "apple" in
    a??le) echo "match" ;;
    *) echo "no" ;;
esac'
assert_stdout "match" \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# Multi-Character Matching
# ==============================================================================
# REQUIREMENT: SHALL-2-14-2-490: The <asterisk> ('*') is a pattern that shall
# match any string, including the null string.
# REQUIREMENT: SHALL-2-14-1-487: *An <asterisk> is a pattern that shall match
# multiple characters...

test_cmd='
case "apple" in
    a*e) echo "match_1" ;;
esac
case "apple" in
    apple*) echo "match_2" ;;
esac'
assert_stdout "match_1
match_2" \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# Bracket Expressions
# ==============================================================================
# REQUIREMENT: SHALL-2-14-1-488: [A <left-square-bracket> shall introduce a
# bracket expression if the characters following it meet the...
# REQUIREMENT: SHALL-2-14-1-489: A <left-square-bracket> that does not
# introduce a valid bracket expression shall match the character...
# REQUIREMENT: SHALL-2-14-1-477: The pattern bracket expression also shall
# match a single collating element.

test_cmd='
case "b" in
    [abc]) echo "match_bracket" ;;
esac
case "[" in
    [) echo "match_literal" ;;
esac'
assert_stdout "match_bracket
match_literal" \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# Escaping and Quoting
# ==============================================================================
# REQUIREMENT: SHALL-2-14-1-478: In a pattern, or part of one, where a
# shell-quoting <backslash> can be used, a <backslash> character...
# REQUIREMENT: SHALL-2-14-1-479: A <backslash> character that is not inside a
# bracket expression shall preserve the literal value of the following...
# REQUIREMENT: SHALL-2-14-1-480: All of the requirements and effects of quoting
# on ordinary, shell special, and special pattern characters...
# REQUIREMENT: SHALL-2-14-1-483: If any character (ordinary, shell special, or
# pattern special) is quoted, or escaped with a <backslash>...
# REQUIREMENT: SHALL-2-14-1-484: The application shall ensure that it quotes
# or escapes any character that would otherwise be treated...

# We test that a quoted `*` matches exactly a literal `*`.
test_cmd='
case "a*b" in
    "a*b") echo "quoted" ;;
esac
case "a*b" in
    a\*b) echo "escaped" ;;
esac'
assert_stdout "quoted
escaped" \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# Patterns Matching Multiple Characters
# ==============================================================================
# REQUIREMENT: SHALL-2-14-2-491: The concatenation of patterns matching a
# single character is a valid pattern that shall match the co...
# REQUIREMENT: SHALL-2-14-2-492: In such patterns, each <asterisk> shall match
# a string of zero or more characters, matching the grea...

test_cmd='
case "xyzabc" in
    x*c) echo yes ;;
    *) echo no ;;
esac
'
assert_stdout "yes" \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# Pathname Expansion Rules
# ==============================================================================
# REQUIREMENT: SHALL-2-14-3-493: The <slash> character in a pathname shall be
# explicitly matched by using one or more <slash> characters...
# REQUIREMENT: SHALL-2-14-3-494: <slash> characters in the pattern shall be
# identified before bracket expressions; thus, a <slash> cannot be...
# REQUIREMENT: SHALL-2-14-3-495: If a <slash> character is found following an
# unescaped <left-square-bracket> character before a corresponding...

# Create a controlled directory structure.
mkdir -p tmp_pattern/dir
touch tmp_pattern/dir/file.txt tmp_pattern/dir/file.md tmp_pattern/a.txt

# `*` cannot match across a `/`.
test_cmd='echo tmp_pattern/*/file.txt'
assert_stdout "tmp_pattern/dir/file.txt" \
    "$TARGET_SHELL -c '$test_cmd'"

test_cmd='echo tmp_pattern/*file.txt'
assert_stdout "tmp_pattern/*file.txt" \
    "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-2-14-3-496: If a filename begins with a <period> ('.'),
# the <period> shall be explicitly matched by using a <period>...
# REQUIREMENT: SHALL-2-14-3-497: The leading <period> shall not be matched by:
# The <asterisk> or <question-mark> special characters...

touch tmp_pattern/.hidden
test_cmd='echo tmp_pattern/*'
assert_stdout "tmp_pattern/a.txt tmp_pattern/dir" \
    "$TARGET_SHELL -c '$test_cmd'"

test_cmd='echo tmp_pattern/.*'
assert_stdout "tmp_pattern/. tmp_pattern/.. tmp_pattern/.hidden" \
    "$TARGET_SHELL -c '$test_cmd'"


# REQUIREMENT: SHALL-2-14-3-498: If a specified pattern contains any '*', '?' or
# '[' characters that will be treated as special...
# REQUIREMENT: SHALL-2-14-3-499: Each component that contains any such characters
# shall require read permission in the directory...
# REQUIREMENT: SHALL-2-14-3-500: Any component, except the last, that does not
# contain any '*', '?' or '[' characters...
# REQUIREMENT: SHALL-2-14-3-501: If these permissions are denied, or if an
# attempt to open or search a pathname as a directory...
# REQUIREMENT: SHALL-2-14-3-502: If the pattern matches any existing filenames
# or pathnames, the pattern shall be replaced with those...
# REQUIREMENT: SHALL-2-14-3-503: If this collating sequence does not have a total
# ordering of all characters...
# REQUIREMENT: SHALL-2-14-3-504: If the pattern does not match any existing
# filenames or pathnames, the pattern string shall be left...
# REQUIREMENT: SHALL-2-14-3-505: If a specified pattern does not contain any
# '*', '?' or '[' characters that will be treated as special...

test_cmd='echo tmp_pattern/*.md'
assert_stdout "tmp_pattern/*.md" \
    "$TARGET_SHELL -c '$test_cmd'"


report
