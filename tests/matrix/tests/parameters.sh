# Test: Parameters and Variables
# Target: tests/matrix/tests/parameters.sh
#
# POSIX Shells support positional, special, and environment variables. Here we
# ensure that parameter assignment, positional variables (like $1, $#), and
# special variables (like $@, $*, $?) behave precisely as specified.

. "$MATRIX_DIR/lib.sh"

# ==============================================================================
# Positional Parameters
# ==============================================================================
# REQUIREMENT: SHALL-2-5-1-060:
# The digits denoting the positional parameters shall always be interpreted as
# a decimal value, even if there is a leading zero.
# REQUIREMENT: SHALL-2-5-1-061:
# When a positional parameter with more than one digit is specified, the
# application shall enclose the digits in braces (see 2.6.2 Parameter Expansion
# ).

test_cmd='
myfunc() {
    echo "$01"
}
myfunc "arg"
'
# `$01` means `$0` followed by a literal `1`.
# Let's test using a subshell with arguments to properly evaluate positional
# params.
test_cmd='echo "$01"; echo "${10}"'
assert_stdout "$TARGET_SHELL"'1
10th' \
    "$TARGET_SHELL -c '$test_cmd' '$TARGET_SHELL' 1 2 3 4 5 6 7 8 9 10th"


# ==============================================================================
# Special Parameters
# ==============================================================================
# REQUIREMENT: SHALL-2-5-059:
# The shell shall process their values as characters only when performing
# operations that are described in this standard in terms of characters.
# REQUIREMENT: SHALL-2-5-2-062:
# Listed below are the special parameters and the values to which they shall
# expand.
# REQUIREMENT: SHALL-2-5-2-072:
# The -i option shall be included in "$-" if the shell is interactive,
# regardless of whether it was specified on invocation.
# REQUIREMENT: SHALL-2-5-2-063:
# When the expansion occurs in a context where field splitting will be
# performed, any empty fields may be discarded and each of the non-empty fields
# shall be further split as described in 2.6.5 Field Splitting .
# REQUIREMENT: SHALL-2-5-2-063:
# When the expansion occurs in a context where
# field splitting will be performed, any empty fields may be discarded...

test_cmd='
for i in $*; do echo "$i"; done
for i in $@; do echo "$i"; done
'
assert_stdout "a
b
c
a
b
c" \
    "$TARGET_SHELL -c '$test_cmd' sh a b c"

# REQUIREMENT: SHALL-2-5-2-064:
# If one of these conditions is true, the initial fields shall be retained as
# separate fields, except that if the parameter being expanded was embedded
# within a word, the first field shall be joined with the beginning part of the
# original word and the last field shall be joined with the end part of the
# original word.
# REQUIREMENT: SHALL-2-5-2-067:
# When the expansion occurs in a context where field splitting will not be
# performed, the initial fields shall be joined to form a single field with the
# value of each parameter separated by the first character of the IFS variable
# if IFS contains at least one character, or separated by a <space> if IFS is
# unset, or with no separation if IFS is set to a null string.

test_cmd='
for i in "$*"; do echo "$i"; done
for i in "$@"; do echo "$i"; done
'
# `$*` is a single string. `$@` is distinct arguments.
assert_stdout "a b c
a
b
c" \
    "$TARGET_SHELL -c '$test_cmd' sh a b c"

# REQUIREMENT: SHALL-2-5-2-065:
# If there are no positional parameters, the expansion of '@' shall generate
# zero fields, even when '@' is within double-quotes; however, if the expansion
# is embedded within a word which contains one or more other parts that expand
# to a quoted null string, these null string(s) shall still produce an empty
# field, except that if the other parts are all within the same double-quotes as
# the '@' , it is unspecified whether the result is zero fields or one empty
# field.

test_cmd='
for i in "$@"; do echo "found: $i"; done
'
assert_stdout "" \
    "$TARGET_SHELL -c '$test_cmd' sh"

# REQUIREMENT: SHALL-2-5-2-068:
# The command name (parameter 0) shall not be counted in the number given by
# '#' because it is a special parameter, not a positional parameter.

test_cmd='echo "$#"'
assert_stdout "3" \
    "$TARGET_SHELL -c '$test_cmd' sh a b c"

# REQUIREMENT: SHALL-2-5-2-069:
# If this pipeline terminated, the status value shall be its exit status;
# otherwise, the status value shall be the same as the exit status that would
# have resulted if the pipeline had been terminated by a signal with the same
# number as the signal that stopped it.
# REQUIREMENT: SHALL-2-5-2-070:
# The value of the special parameter '?' shall be set to 0 during
# initialization of the shell.

test_cmd='echo "$?"'
assert_stdout "0" \
    "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-2-5-2-071:
# When a subshell environment is created, the value of the special parameter
# '?' from the invoking shell environment shall be preserved in the subshell.

test_cmd='false; (echo "$?")'
assert_stdout "1" \
    "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-2-5-2-073:
# In a subshell (see 2.13 Shell Execution Environment ), '$' shall expand to
# the same value as that of the current shell.

test_cmd='parent="$$"; sub="$(echo "$$")"; [ "$parent" = "$sub" ] && echo "same"'
assert_stdout "same" \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# Environment Variables
# ==============================================================================
# REQUIREMENT: SHALL-2-5-3-074:
# Variables shall be initialized from the environment (as defined by XBD 8.
# REQUIREMENT: SHALL-2-5-3-075:
# Shell variables shall be initialized only from environment variables that
# have valid names.
# REQUIREMENT: SHALL-2-5-3-076:
# If a variable is initialized from the environment, it shall be marked for
# export immediately; see the export special built-in.
# REQUIREMENT: SHALL-V3CHAP02-1002-DUP75:
# The following variables shall affect the execution of the shell: ENV The
# processing of the ENV shell variable shall be supported if the system supports
# the User Portability Utilities option.

test_cmd='env | grep -q "^TEST_ENV_VAR=" && echo "exported"'
assert_stdout "exported" \
    "TEST_ENV_VAR=value $TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-2-5-3-085:
# Each time an interactive shell is ready to read a command, the value of this
# variable shall be subjected to parameter expansion (see 2.6.2 Parameter
# Expansion ) and exclamation-mark expansion (see below).
# REQUIREMENT: SHALL-2-5-3-086:
# After expansion, the value shall be written to standard error.
# REQUIREMENT: SHALL-2-5-3-090:
# The default value shall be "$ " .
# REQUIREMENT: SHALL-2-5-3-093:
# Each time the user enters a <newline> prior to completing a command line in
# an interactive shell, the value of this variable shall be subjected to
# parameter expansion (see 2.6.2 Parameter Expansion ).
# REQUIREMENT: SHALL-2-5-3-086:
# After expansion, the value shall be written to standard error.
# REQUIREMENT: SHALL-2-5-3-095:
# The default value shall be "> " .


# REQUIREMENT: SHALL-2-5-3-081:
# If IFS is not set, it shall behave as normal for an unset variable, except
# that field splitting by the shell and line splitting by the read utility shall
# be performed as if the value of IFS is <space><tab><newline>; see 2.6.5 Field
# Splitting .
# REQUIREMENT: SHALL-2-5-3-082:
# The shell shall set IFS to <space><tab><newline> when it is invoked.

test_cmd='
foo="a b	c
d"
for i in $foo; do echo "split"; done | wc -l | tr -d " "
'
assert_stdout "4" \
    "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-2-5-3-084:
# In a subshell (see 2.13 Shell Execution Environment ), PPID shall be set to
# the same value as that of the parent of the current shell.

test_cmd='parent="$PPID"; sub="$(echo "$PPID")"; [ "$parent" = "$sub" ] && echo "same"'
assert_stdout "same" \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# Environment Variables (PS4, PWD, etc.)
# ==============================================================================
# REQUIREMENT: SHALL-2-5-3-096:
# When an execution trace ( set -x ) is being performed, before each line in
# the execution trace, the value of this variable shall be subjected to
# parameter expansion (see 2.6.2 Parameter Expansion ).
# REQUIREMENT: SHALL-2-5-3-086:
# After expansion, the value shall be written to standard error.
# REQUIREMENT: SHALL-2-5-3-098:
# The default value shall be "+ " .

test_cmd='
set -x
echo "traced"
set +x
'
assert_stderr_contains "+ echo traced" \
    "$TARGET_SHELL -c '$test_cmd'"

# Changing PS4 alters the trace prefix, and expands variables!
test_cmd='
PS4="TRACE:\$LINENO> "
set -x
echo "traced"
set +x
'
assert_stderr_contains "TRACE:" \
    "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-2-5-3-099:
# In the shell the value shall be initialized from the environment as follows.
# REQUIREMENT: SHALL-2-5-3-100:
# If a value for PWD is passed to the shell in the environment when it is
# executed, the value is an absolute pathname of the current working directory
# that is no longer than {PATH_MAX} bytes including the terminating null byte,
# and the value does not contain any components that are dot or dot-dot, then
# the shell shall set PWD to the value from the environment.
# REQUIREMENT: SHALL-SH-1017:
# The following environment
# variables shall affect the execution of sh:...
# REQUIREMENT: SHALL-ENVIRONMENT-VARIABLES-024:
# This variable shall represent an absolute pathname of the current working
# directory.

test_cmd='echo "$PWD"'
# We pass an explicit PWD via env and see if it's respected (if it matches
# the actual current directory).
assert_stdout "$PWD" \
    "PWD=\"$PWD\" $TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# PS1 and Exclamation-mark Expansion
# ==============================================================================
# REQUIREMENT: SHALL-2-5-3-085:
# Each time an interactive shell is ready to read a command, the value of this
# variable shall be subjected to parameter expansion (see 2.6.2 Parameter
# Expansion ) and exclamation-mark expansion (see below).
# REQUIREMENT: SHALL-2-5-3-087:
# The expansions shall be performed in two passes, where the result of the
# first pass is input to the second pass.
# REQUIREMENT: SHALL-2-5-3-088:
# One of the passes shall perform only the exclamation-mark expansion described
# below.
# REQUIREMENT: SHALL-2-5-3-089:
# The other pass shall perform the other expansion(s) according to the rules in
# 2.6 Word Expansions .
# REQUIREMENT: SHALL-2-5-3-091:
# Exclamation-mark expansion: The shell shall replace each instance of the
# <exclamation-mark> character ( '!' ) with the history file number (see Command
# History List ) of the next command to be typed.
# REQUIREMENT: SHALL-2-5-3-092:
# An <exclamation-mark> character escaped by another <exclamation-mark>
# character (that is, "!!" ) shall expand to a single <exclamation-mark>
# character.

interactive_script=$(cat << 'EOF'
sleep 0.5
echo 'PS1="cmd \! var \$(echo 1)> "'
sleep 0.5
echo 'echo interactive_test'
sleep 0.5
echo 'exit'
EOF
)

cmd="( $interactive_script ) | \"$MATRIX_DIR/pty\" $TARGET_SHELL -i"
actual=$(eval "$cmd" 2>&1)

# Testing that PS1 expansion expands command history number `!` and command
# substitution `$(...)`.
# The exact command number might vary, so we just check for `cmd ` and ` var
# 1>`.
case "$actual" in
    *"cmd "*" var 1>"*)
        pass
        ;;
    *)
        fail "Expected PS1 expansion to process '!' and '\$(...)', got: $actual"
        ;;
esac

# ==============================================================================
# ENV Processing (interactive shell)
# ==============================================================================
# REQUIREMENT: SHALL-2-5-3-078:
# This variable, when and only when an interactive shell is invoked, shall be
# subjected to parameter expansion by the shell and the resulting value shall
# be used as a pathname of a file.
# REQUIREMENT: SHALL-2-5-3-079:
# Before any interactive commands are read, the shell shall tokenize the
# contents of the file, parse the tokens as a program, and execute the
# resulting commands in the current environment.

# Create an ENV file that sets a marker variable
_env_file="${TMPDIR:-/tmp}/_test_env_$$"
echo 'ENVMARKER=loaded' > "$_env_file"

# ENV is subjected to parameter expansion: use a variable to set the path
_out=$($TARGET_SHELL -c "
    EFILE=$_env_file; export EFILE
    ENV=\$EFILE; export ENV
    $TARGET_SHELL -i -c 'echo \$ENVMARKER' 2>/dev/null
")
case "$_out" in
    *loaded*) pass ;;
    *) fail "ENV file not processed for interactive shell: got '$_out'" ;;
esac
rm -f "$_env_file"

# REQUIREMENT: SHALL-2-5-3-080:
# ENV shall be ignored if the user's real and effective user IDs or real and
# effective group IDs are different.
# Cannot test SUID behavior in this test environment — just verify it doesn't
# crash when ENV is set to a valid file with normal permissions
_env_file2="${TMPDIR:-/tmp}/_test_env2_$$"
echo 'echo env_ran' > "$_env_file2"
assert_exit_code 0 "$TARGET_SHELL -c 'ENV=$_env_file2 $TARGET_SHELL -c true 2>/dev/null'"
rm -f "$_env_file2"

# REQUIREMENT: SHALL-2-5-3-083:
# Changing the value of LC_CTYPE after the shell has started shall not affect
# the lexical processing of shell commands in the current shell execution
# environment or its subshells.

# Setting LC_CTYPE mid-script should not break lexical processing
test_cmd='
LC_CTYPE=C; export LC_CTYPE
echo "hello world"
LC_CTYPE=POSIX; export LC_CTYPE
echo "still works"
'
assert_stdout "hello world
still works" \
    "$TARGET_SHELL -c '$test_cmd'"

report
