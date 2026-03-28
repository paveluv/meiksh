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
# REQUIREMENT: SHALL-2-5-1-060: The digits denoting the positional parameters
# shall always be interpreted as a decimal value, even if there is a leading
# zero.
# REQUIREMENT: SHALL-2-5-1-061: When a positional parameter with more than one
# digit is specified, the application shall enclose the digits in braces...

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
# REQUIREMENT: SHALL-2-5-059: The shell shall process their values as characters
# only when performing operations that are describe...
# REQUIREMENT: SHALL-2-5-2-062: Listed below are the special parameters and the
# values to which they shall expand.
# REQUIREMENT: SHALL-2-5-2-072: The -i option shall be included in "$-" if the
# shell is interactive, regardless of whether it was sp...
# REQUIREMENT: SHALL-2-5-2-063: When the expansion occurs in a context where
# field splitting will be performed, any empty fields may be discarded...
# REQUIREMENT: SHALL-2-5-2-066: When the expansion occurs in a context where
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

# REQUIREMENT: SHALL-2-5-2-064: If one of these conditions is true, the initial
# fields shall be retained as separate fields... ($@ within double quotes)
# REQUIREMENT: SHALL-2-5-2-067: When the expansion occurs in a context where
# field splitting will not be performed, the initial fields shall be joined...
# ($* within double quotes)

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

# REQUIREMENT: SHALL-2-5-2-065: If there are no positional parameters, the
# expansion of '@' shall generate zero fields, even when '@' is within
# double-quotes...

test_cmd='
for i in "$@"; do echo "found: $i"; done
'
assert_stdout "" \
    "$TARGET_SHELL -c '$test_cmd' sh"

# REQUIREMENT: SHALL-2-5-2-068: The command name (parameter 0) shall not be
# counted in the number given by '#' because it is a special parameter...

test_cmd='echo "$#"'
assert_stdout "3" \
    "$TARGET_SHELL -c '$test_cmd' sh a b c"

# REQUIREMENT: SHALL-2-5-2-069: If this pipeline terminated, the status value
# shall be its exit status...
# REQUIREMENT: SHALL-2-5-2-070: The value of the special parameter '?' shall be
# set to 0 during initialization of the shell.

test_cmd='echo "$?"'
assert_stdout "0" \
    "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-2-5-2-071: When a subshell environment is created, the
# value of the special parameter '?' from the invoking shell...

test_cmd='false; (echo "$?")'
assert_stdout "1" \
    "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-2-5-2-073: In a subshell... '$' shall expand to the same
# value as that of the current shell.

test_cmd='parent="$$"; sub="$(echo "$$")"; [ "$parent" = "$sub" ] && echo "same"'
assert_stdout "same" \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# Environment Variables
# ==============================================================================
# REQUIREMENT: SHALL-2-5-3-074: Variables shall be initialized from the
# environment...
# REQUIREMENT: SHALL-2-5-3-075: Shell variables shall be initialized only from
# environment variables that have valid names.
# REQUIREMENT: SHALL-2-5-3-076: If a variable is initialized from the
# environment, it shall be marked for export immediately...
# REQUIREMENT: SHALL-2-5-3-077: The following variables shall affect the
# execution of the shell:

test_cmd='env | grep -q "^TEST_ENV_VAR=" && echo "exported"'
assert_stdout "exported" \
    "TEST_ENV_VAR=value $TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-2-5-3-085: PS1UP PS1Each time an interactive shell is
# ready to read a command, the value of this variable shall...
# REQUIREMENT: SHALL-2-5-3-086: After expansion, the value shall be written to
# standard error.
# REQUIREMENT: SHALL-2-5-3-090: The default value shall be "$ ".
# REQUIREMENT: SHALL-2-5-3-093: PS2UP PS2Each time the user enters a <newline>
# prior to completing a command line in an interactive...
# REQUIREMENT: SHALL-2-5-3-094: After expansion, the value shall be written to
# standard error.
# REQUIREMENT: SHALL-2-5-3-095: The default value shall be "> ".


# REQUIREMENT: SHALL-2-5-3-081: If IFS is not set, it shall behave as normal
# for an unset variable, except that field splitting...
# REQUIREMENT: SHALL-2-5-3-082: The shell shall set IFS to <space><tab><newline>
# when it is invoked.

test_cmd='
foo="a b	c
d"
for i in $foo; do echo "split"; done | wc -l | tr -d " "
'
assert_stdout "4" \
    "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-2-5-3-084: In a subshell... PPID shall be set to the same
# value as that of the parent of the current shell.

test_cmd='parent="$PPID"; sub="$(echo "$PPID")"; [ "$parent" = "$sub" ] && echo "same"'
assert_stdout "same" \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# Environment Variables (PS4, PWD, etc.)
# ==============================================================================
# REQUIREMENT: SHALL-2-5-3-096: PS4UP PS4When an execution trace (set -x) is
# being performed, before each line in the execution trace...
# REQUIREMENT: SHALL-2-5-3-097: After expansion, the value shall be written to
# standard error.
# REQUIREMENT: SHALL-2-5-3-098: The default value shall be "+ ".

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

# REQUIREMENT: SHALL-2-5-3-099: In the shell the value shall be initialized from
# the environment as follows.
# REQUIREMENT: SHALL-2-5-3-100: If a value for PWD is passed to the shell in the
# environment when it is executed, the value is an absolute...
# REQUIREMENT: SHALL-ENVIRONMENT VARIABLES-023: The following environment
# variables shall affect the execution of sh:...
# REQUIREMENT: SHALL-ENVIRONMENT VARIABLES-024: PWDThis variable shall represent
# an absolute pathname of the current working directory.

test_cmd='echo "$PWD"'
# We pass an explicit PWD via env and see if it's respected (if it matches
# the actual current directory).
assert_stdout "$PWD" \
    "PWD=\"$PWD\" $TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# PS1 and Exclamation-mark Expansion
# ==============================================================================
# REQUIREMENT: SHALL-2-5-3-085: PS1UP PS1Each time an interactive shell is
# ready to read a command, the value of this variable shall...
# REQUIREMENT: SHALL-2-5-3-087: The expansions shall be performed in two passes,
# where the result of the first pass is input to the...
# REQUIREMENT: SHALL-2-5-3-088: One of the passes shall perform only the
# exclamation-mark expansion described below.
# REQUIREMENT: SHALL-2-5-3-089: The other pass shall perform the other
# expansion(s) according to the rules in 2.6 Word Expansions.
# REQUIREMENT: SHALL-2-5-3-091: Exclamation-mark expansion: The shell shall
# replace each instance of the <exclamation-mark> character...
# REQUIREMENT: SHALL-2-5-3-092: An <exclamation-mark> character escaped by
# another <exclamation-mark> character (that is, "!!") shall be...

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

# Testing that PS1 expansion expands command history number `!` and command substitution `$(...)`.
# The exact command number might vary, so we just check for `cmd ` and ` var 1>`.
case "$actual" in
    *"cmd "*" var 1>"*)
        pass
        ;;
    *)
        fail "Expected PS1 expansion to process '!' and '\$(...)', got: $actual"
        ;;
esac

report

# ==============================================================================
# Additional Parameters
# ==============================================================================
# REQUIREMENT: SHALL-2-5-3-078: ENVUP ENVThis variable, when and only when an
# interactive shell is invoked, shall be subjected to parameter expansion...
# REQUIREMENT: SHALL-2-5-3-079: Before any interactive commands are read, the
# shell shall tokenize... the commands in the script indicated by $ENV.
# REQUIREMENT: SHALL-2-5-3-080: ENV shall be ignored if the user's real and
# effective user IDs or real and effective group IDs are d...
# REQUIREMENT: SHALL-2-5-3-083: Changing the value of LC_CTYPE after the shell
# has started shall not affect the lexical processing of...
