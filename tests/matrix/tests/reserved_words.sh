# Test: Reserved Words
# Target: tests/matrix/tests/reserved_words.sh
#
# Reserved words define the core structure of the POSIX Shell language. They
# are only recognized under strict conditions, ensuring they do not interfere
# with standard commands when quoted or used as arguments.

. "$MATRIX_DIR/lib.sh"

# ==============================================================================
# Unquoted Recognition
# ==============================================================================
# REQUIREMENT: SHALL-2-4-056:
# The following words shall be recognized as
# reserved words: case, do, done, elif, else, esac, fi, for, if, in, then,
# until, while...
# REQUIREMENT: SHALL-2-4-057:
# This recognition shall only occur when none of the
# characters is quoted and when the word is used as: The first word of a
# command... The third word in a case or for command.

# If a reserved word like `if` is quoted, it ceases to be reserved and is simply
# treated as a command name.
test_cmd='"if" true; then echo yes; fi'
assert_exit_code_non_zero \
    "$TARGET_SHELL -c '$test_cmd'"

# When used as the first word of a command unquoted, it builds syntax.
test_cmd='if true; then echo "yes"; fi'
assert_stdout "yes" \
    "$TARGET_SHELL -c '$test_cmd'"

# A reserved word can be safely used as an argument to another command because
# it is not the first word of a command.
test_cmd='echo if while for done'
assert_stdout "if while for done" \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# The 'time' Reserved Word
# ==============================================================================
# REQUIREMENT: SHALL-2-4-058:
# When used in circumstances where reserved words are recognized (described
# above), the following words may be recognized as reserved words, in which case
# the results are unspecified except as described below for time : [[ ]]
# function namespace select time When the word time is recognized as a reserved
# word in circumstances where it would, if it were not a reserved word, be the
# command name (see 2.9.1.1 Order of Processing ) of a simple command that would
# execute the time utility in a manner other than one for which time states that
# the results are unspecified, the behavior shall be as specified for the time
# utility.

# The `time` command is special because it measures execution, but it's treated
# as a reserved word or executed transparently.
test_cmd='time echo "measured"'
assert_stdout "measured" \
    "$TARGET_SHELL -c '$test_cmd'"


report
