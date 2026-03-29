# Test: getopts — Parse Utility Options
# Target: tests/matrix/tests/getopts.sh
#
# Tests the getopts built-in utility for POSIX compliance. getopts is used by
# shell scripts to parse positional parameters as options. It supports options
# with and without arguments, silent error reporting, OPTIND/OPTARG tracking,
# and proper termination when all options have been consumed.
# REQUIREMENT: SHALL-GETOPTS-1174:

. "$MATRIX_DIR/lib.sh"

# ==============================================================================
# Basic Option Character Assignment
# ==============================================================================
# REQUIREMENT: SHALL-GETOPTS-1198:
# name operand — the shell variable specified by name shall be set to the
# option character that was found.

test_cmd='OPTIND=1; getopts ab: name -a; echo "$name"'
assert_stdout "a" \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# Basic Option Parsing Loop with Single Options
# ==============================================================================
# REQUIREMENT: SHALL-GETOPTS-1175:
# getopts shall be suitable for use in a loop to extract each option in turn.

test_cmd='
result=""
OPTIND=1
set -- -a -b -c
while getopts abc name; do
    result="${result}${name}"
done
echo "$result"
'
assert_stdout "abc" \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# Options with Arguments (: in optstring)
# ==============================================================================
# REQUIREMENT: SHALL-GETOPTS-1194:
# REQUIREMENT: SHALL-GETOPTS-1050:
# If a character is followed by a <colon>, the option shall be expected to
# have an argument, which should be supplied as a separate argument.

test_cmd='
OPTIND=1
set -- -f myfile
getopts f: name
echo "$name"
'
assert_stdout "f" \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# OPTARG Set to Option-Argument Value
# ==============================================================================
# REQUIREMENT: SHALL-GETOPTS-1197:
# Whenever getopts is invoked, it shall place the value of the next
# option-argument in the shell variable OPTARG.

test_cmd='
OPTIND=1
set -- -f myfile.txt
getopts f: name
echo "$OPTARG"
'
assert_stdout "myfile.txt" \
    "$TARGET_SHELL -c '$test_cmd'"

# Option argument concatenated directly with the option letter
test_cmd='
OPTIND=1
set -- -fmyfile.txt
getopts f: name
echo "$OPTARG"
'
assert_stdout "myfile.txt" \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# OPTIND Tracking
# ==============================================================================
# REQUIREMENT: SHALL-GETOPTS-1191:
# REQUIREMENT: SHALL-GETOPTS-1178:
# OPTIND shall be initialized to 1 when the shell is invoked.
# After each successful invocation, OPTIND shall be updated to point to
# the next argument to be processed.

test_cmd='echo "$OPTIND"'
assert_stdout "1" \
    "$TARGET_SHELL -c '$test_cmd'"

test_cmd='
OPTIND=1
set -- -a -b
getopts ab name
echo "$OPTIND"
'
assert_stdout "2" \
    "$TARGET_SHELL -c '$test_cmd'"

test_cmd='
OPTIND=1
set -- -a -b
getopts ab name
getopts ab name
echo "$OPTIND"
'
assert_stdout "3" \
    "$TARGET_SHELL -c '$test_cmd'"

# OPTIND after an option with an argument (separate word)
test_cmd='
OPTIND=1
set -- -f val
getopts f: name
echo "$OPTIND"
'
assert_stdout "3" \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# Error Reporting for Invalid Option
# ==============================================================================
# REQUIREMENT: SHALL-GETOPTS-1205:
# REQUIREMENT: SHALL-GETOPTS-1206:
# REQUIREMENT: SHALL-GETOPTS-1185:
# If an option character not contained in optstring is found, the shell
# variable specified by name shall be set to the <question-mark> character.
# The shell variable OPTARG shall be set to the option character found.

test_cmd='
OPTIND=1
set -- -z
getopts ab name
echo "$name"
'
assert_stdout "?" \
    "$TARGET_SHELL -c '$test_cmd'"

# Verify OPTARG is set to the offending character (silent mode)
test_cmd='
OPTIND=1
set -- -z
getopts :ab name
echo "$OPTARG"
'
assert_stdout "z" \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# Error Reporting for Missing Argument
# ==============================================================================
# REQUIREMENT: SHALL-GETOPTS-1207:
# REQUIREMENT: SHALL-GETOPTS-1187:
# REQUIREMENT: SHALL-GETOPTS-1188:
# If an option requiring an argument is found, but the option-argument is
# not supplied, the name variable shall be set to <question-mark>.

test_cmd='
OPTIND=1
set -- -f
getopts f: name 2>/dev/null
echo "$name"
'
assert_stdout "?" \
    "$TARGET_SHELL -c '$test_cmd'"

# In silent mode (: prefix), name shall be set to : for missing argument
# REQUIREMENT: SHALL-GETOPTS-1186:
# If optstring starts with :, name shall be set to : when the
# option-argument is missing, and OPTARG shall be set to the option character.
test_cmd='
OPTIND=1
set -- -f
getopts :f: name
echo "$name:$OPTARG"
'
assert_stdout "::f" \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# Multiple Invocations with OPTIND Reset
# ==============================================================================
# REQUIREMENT: SHALL-GETOPTS-1202:
# Multiple sets of options can be parsed by resetting OPTIND to 1 between
# invocations.

test_cmd='
OPTIND=1
set -- -a -b
result=""
while getopts ab name; do
    result="${result}${name}"
done
OPTIND=1
set -- -x -y
while getopts xy name; do
    result="${result}${name}"
done
echo "$result"
'
assert_stdout "abxy" \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# -- End-of-Options Handling
# ==============================================================================
# REQUIREMENT: SHALL-GETOPTS-1190:
# The special option -- shall be recognized as the end of options.
# getopts shall stop processing and return non-zero.

test_cmd='
OPTIND=1
result=""
set -- -a -- -b
while getopts ab name; do
    result="${result}${name}"
done
echo "$result"
'
assert_stdout "a" \
    "$TARGET_SHELL -c '$test_cmd'"

# OPTIND should point to the first argument after --
test_cmd='
OPTIND=1
set -- -a -- operand
while getopts a name; do :; done
shift $((OPTIND - 1))
echo "$1"
'
assert_stdout "operand" \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# Non-Zero Return When All Options Processed
# ==============================================================================
# REQUIREMENT: SHALL-GETOPTS-1189:
# REQUIREMENT: SHALL-GETOPTS-1192:
# REQUIREMENT: SHALL-GETOPTS-1052:
# When the end of options is encountered, the getopts utility shall return
# with a return value greater than zero.

test_cmd='
OPTIND=1
set -- -a
getopts a name
getopts a name
echo "$?"
'
assert_stdout "1" \
    "$TARGET_SHELL -c '$test_cmd'"

assert_exit_code 0 \
    "$TARGET_SHELL -c 'OPTIND=1; set -- -a; getopts a name'"

assert_exit_code_non_zero \
    "$TARGET_SHELL -c 'OPTIND=1; set -- ; getopts a name'"

# ==============================================================================
# Silent Error Reporting (optstring starts with :)
# ==============================================================================
# REQUIREMENT: SHALL-GETOPTS-1204:
# If the first character of optstring is a <colon>, the shell variable name
# shall be set to the <colon> character for a missing option-argument and
# to ? for an unknown option. No error messages shall be written.

# Unknown option in silent mode — no stderr output
test_cmd='
OPTIND=1
set -- -z
getopts :ab name
echo "$name:$OPTARG"
'
assert_stdout "?:z" \
    "$TARGET_SHELL -c '$test_cmd'"

# Verify no diagnostic is written to stderr in silent mode
assert_stderr_empty \
    "$TARGET_SHELL -c 'OPTIND=1; set -- -z; getopts :ab name'"

# Missing argument in silent mode
test_cmd='
OPTIND=1
set -- -f
getopts :f: name
echo "$name:$OPTARG"
'
assert_stdout "::f" \
    "$TARGET_SHELL -c '$test_cmd'"

assert_stderr_empty \
    "$TARGET_SHELL -c 'OPTIND=1; set -- -f; getopts :f: name'"

# ==============================================================================
# OPTIND=1 Reset Between Invocations
# ==============================================================================
# REQUIREMENT: SHALL-GETOPTS-1202:
# If the application sets OPTIND to 1, a new set of parameters can be parsed.

test_cmd='
OPTIND=1
set -- -x
getopts x name
r1="$name"
OPTIND=1
set -- -y
getopts y name
r2="$name"
echo "${r1}:${r2}"
'
assert_stdout "x:y" \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# Combined Short Options
# ==============================================================================
# REQUIREMENT: SHALL-GETOPTS-1196:
# Multiple options may be combined after a single hyphen.

test_cmd='
OPTIND=1
result=""
set -- -abc
while getopts abc name; do
    result="${result}${name}"
done
echo "$result"
'
assert_stdout "abc" \
    "$TARGET_SHELL -c '$test_cmd'"

# Combined options where the last takes an argument
test_cmd='
OPTIND=1
result=""
arg=""
set -- -abf file.txt
while getopts abf: name; do
    result="${result}${name}"
    if [ "$name" = "f" ]; then arg="$OPTARG"; fi
done
echo "${result}:${arg}"
'
assert_stdout "abf:file.txt" \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# OPTARG Unset for Options Without Arguments
# ==============================================================================
# REQUIREMENT: SHALL-GETOPTS-1182:
# If no option was found, or if the option does not have an option-argument,
# OPTARG shall be unset.

test_cmd='
OPTIND=1
OPTARG="stale"
set -- -a
getopts a name
echo "${OPTARG:-UNSET}"
'
assert_stdout "UNSET" \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# Default Positional Parameters Used
# ==============================================================================
# REQUIREMENT: SHALL-GETOPTS-1199:
# By default, the list of parameters parsed by the getopts utility shall be
# the positional parameters currently set in the invoking shell environment.

test_cmd='
set -- -a -b
OPTIND=1
result=""
while getopts ab name; do
    result="${result}${name}"
done
echo "$result"
'
assert_stdout "ab" \
    "$TARGET_SHELL -c '$test_cmd'"

# Verify that getopts uses "$@" by default when no args operand is given
test_cmd='
f() {
    OPTIND=1
    result=""
    while getopts xy name; do
        result="${result}${name}"
    done
    echo "$result"
}
f -x -y
'
assert_stdout "xy" \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# Non-Option Argument Stops Parsing
# ==============================================================================
# REQUIREMENT: SHALL-GETOPTS-1200:
# The first operand that does not begin with - or is the argument -- signals
# the end of options.

test_cmd='
OPTIND=1
result=""
set -- -a operand -b
while getopts ab name; do
    result="${result}${name}"
done
echo "$result"
'
assert_stdout "a" \
    "$TARGET_SHELL -c '$test_cmd'"

report
