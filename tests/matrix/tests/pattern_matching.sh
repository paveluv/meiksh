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
# REQUIREMENT: SHALL-2-14-1-486:
# A <question-mark> is a pattern that shall match any character.
# REQUIREMENT: SHALL-2-14-1-481:
# An ordinary character is a pattern that shall match itself.
# REQUIREMENT: SHALL-2-14-1-476:
# The following patterns shall match a single character: ordinary characters,
# special pattern characters, and pattern bracket expressions.
# REQUIREMENT: SHALL-2-14-1-482:
# Matching shall be based on the bit pattern used for encoding the character,
# not on the graphic representation of the character.
# REQUIREMENT: SHALL-2-14-1-478:
# When unquoted, unescaped, and not inside a
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
# REQUIREMENT: SHALL-2-14-2-490:
# The following rules are used to construct patterns matching multiple
# characters from patterns matching a single character: The <asterisk> ( '*' )
# is a pattern that shall match any string, including the null string.
# REQUIREMENT: SHALL-2-14-1-487:
# * An <asterisk> is a pattern that shall match multiple characters, as
# described in 2.14.2 Patterns Matching Multiple Characters .

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
# REQUIREMENT: SHALL-2-14-1-488:
# [ A <left-square-bracket> shall introduce a bracket expression if the
# characters following it meet the requirements for bracket expressions stated
# in XBD 9.3.5 RE Bracket Expression , except that the <exclamation-mark>
# character ( '!' ) shall replace the <circumflex> character ( '^' ) in its role
# in a non-matching list in the regular expression notation.
# REQUIREMENT: SHALL-2-14-1-489:
# A <left-square-bracket> that does not introduce a valid bracket expression
# shall match the character itself.
# REQUIREMENT: SHALL-2-14-1-477:
# The pattern bracket expression also shall match a single collating element.

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
# REQUIREMENT: SHALL-2-14-1-478:
# In a pattern, or part of one, where a shell-quoting <backslash> can be used,
# a <backslash> character shall escape the following character as described in
# 2.2.1 Escape Character (Backslash) , regardless of whether or not the
# <backslash> is inside a bracket expression. (The sequence "\\" represents one
# literal <backslash>.)
# REQUIREMENT: SHALL-2-14-1-479:
# In a pattern, or part of one, where a shell-quoting <backslash> cannot be
# used to preserve the literal value of a character that would otherwise be
# treated as special: A <backslash> character that is not inside a bracket
# expression shall preserve the literal value of the following character, unless
# the following character is in a part of the pattern where shell quoting can be
# used and is a shell quoting character, in which case the behavior is
# unspecified.
# REQUIREMENT: SHALL-2-14-1-480:
# All of the requirements and effects of quoting on ordinary, shell special,
# and special pattern characters shall apply to escaping in this context, except
# where specified otherwise. (Situations where this applies include word
# expansions when a pattern used in pathname expansion is not present in the
# original word but results from an earlier expansion, or the argument to the
# find - name or - path primary as passed to find , or the pattern argument to
# the fnmatch () and glob () functions when FNM_NOESCAPE or GLOB_NOESCAPE is not
# set in flags , respectively.)
# REQUIREMENT: SHALL-2-14-1-483:
# If any character (ordinary, shell special, or pattern special) is quoted, or
# escaped with a <backslash>, that pattern shall match the character itself.
# REQUIREMENT: SHALL-2-14-1-484:
# The application shall ensure that it quotes or escapes any character that
# would otherwise be treated as special, in order for it to be matched as an
# ordinary character.

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
# REQUIREMENT: SHALL-2-14-2-491:
# The concatenation of patterns matching a single character is a valid pattern
# that shall match the concatenation of the single characters or collating
# elements matched by each of the concatenated patterns.
# REQUIREMENT: SHALL-2-14-2-492:
# In such patterns, each <asterisk> shall match a string of zero or more
# characters, matching the greatest possible number of characters that still
# allows the remainder of the pattern to match the string.

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
# REQUIREMENT: SHALL-2-14-3-493:
# The rules described so far in 2.14.1 Patterns Matching a Single Character and
# 2.14.2 Patterns Matching Multiple Characters are qualified by the following
# rules that apply when pattern matching notation is used for filename
# expansion: The <slash> character in a pathname shall be explicitly matched by
# using one or more <slash> characters in the pattern; it shall neither be
# matched by the <asterisk> or <question-mark> special characters nor by a
# bracket expression. <slash> characters in the pattern shall be identified
# before bracket expressions; thus, a <slash> cannot be included in a pattern
# bracket expression used for filename expansion.
# REQUIREMENT: SHALL-2-14-3-494:
# <slash> characters in the pattern shall be identified before bracket
# expressions; thus, a <slash> cannot be included in a pattern bracket
# expression used for filename expansion.
# REQUIREMENT: SHALL-2-14-3-495:
# If a <slash> character is found following an unescaped <left-square-bracket>
# character before a corresponding <right-square-bracket> is found, the open
# bracket shall be treated as an ordinary character.
# character.

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

# REQUIREMENT: SHALL-2-14-3-496:
# If a filename begins with a <period> ( '.' ), the <period> shall be
# explicitly matched by using a <period> as the first character of the pattern
# or immediately following a <slash> character.
# REQUIREMENT: SHALL-2-14-3-497:
# The leading <period> shall not be matched by: The <asterisk> or
# <question-mark> special characters

touch tmp_pattern/.hidden
test_cmd='echo tmp_pattern/*'
assert_stdout "tmp_pattern/a.txt tmp_pattern/dir" \
    "$TARGET_SHELL -c '$test_cmd'"

test_cmd='echo tmp_pattern/.*'
assert_stdout "tmp_pattern/. tmp_pattern/.. tmp_pattern/.hidden" \
    "$TARGET_SHELL -c '$test_cmd'"


# REQUIREMENT: SHALL-2-14-3-498:
# If a specified pattern contains any '*' , '?' or '[' characters that will be
# treated as special (see 2.14.1 Patterns Matching a Single Character ), it
# shall be matched against existing filenames and pathnames, as appropriate; if
# directory entries for dot and dot-dot exist, they may be ignored.
# REQUIREMENT: SHALL-2-14-3-499:
# Each component that contains any such characters shall require read
# permission in the directory containing that component.
# REQUIREMENT: SHALL-2-14-3-500:
# Any component, except the last, that does not contain any '*' , '?' or '['
# characters that will be treated as special shall require search permission.
# REQUIREMENT: SHALL-2-14-3-501:
# If these permissions are denied, or if an attempt to open or search a
# pathname as a directory, or an attempt to read an opened directory, fails
# because of an error condition that is related to file system contents, this
# shall not be considered an error and pathname expansion shall continue as if
# the pathname had named an existing directory which had been successfully
# opened and read, or searched, and no matching directory entries had been found
# in it.
# REQUIREMENT: SHALL-2-14-3-502:
# If the pattern matches any existing filenames or pathnames, the pattern shall
# be replaced with those filenames and pathnames, sorted according to the
# collating sequence in effect in the current locale.
# REQUIREMENT: SHALL-2-14-3-503:
# If this collating sequence does not have a total ordering of all characters
# (see XBD 7.3.2 LC_COLLATE ), any filenames or pathnames that collate equally
# shall be further compared byte-by-byte using the collating sequence for the
# POSIX locale.
# REQUIREMENT: SHALL-2-14-3-504:
# If the pattern does not match any existing filenames or pathnames, the
# pattern string shall be left unchanged.
# REQUIREMENT: SHALL-2-14-3-505:
# If a specified pattern does not contain any '*' , '?' or '[' characters that
# will be treated as special, the pattern string shall be left unchanged.

test_cmd='echo tmp_pattern/*.md'
assert_stdout "tmp_pattern/*.md" \
    "$TARGET_SHELL -c '$test_cmd'"


report
