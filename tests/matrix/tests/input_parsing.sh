#!/bin/sh

# Test: Input and Parsing Utilities
# Target: tests/matrix/tests/input_parsing.sh
#
# Tests the read and getopts built-in utilities.

. "$MATRIX_DIR/lib.sh"

# REQUIREMENT: SHALL-READ-1282:
# The read utility shall conform to XBD 12.2 Utility Syntax Guidelines .
# REQUIREMENT: SHALL-COMMAND-1037:
# The following options shall be supported: -p Perform the command search using
# a default value for PATH that is guaranteed to find all of the standard
# utilities.
# REQUIREMENT: SHALL-READ-1284:
# The following options shall be supported: -d delim If delim consists of one
# single-byte character, that byte shall be used as the logical line delimiter.
# REQUIREMENT: SHALL-READ-1285:
# If delim is the null string, the logical line delimiter shall be the null
# byte.
# REQUIREMENT: SHALL-BG-1029:
# The following operand shall be supported: job_id Specify the job to be
# resumed as a background job.
# REQUIREMENT: SHALL-READ-1287:
# If the -d delim option is not specified, or if it is specified and delim is
# not the null string, the standard input shall contain zero or more bytes
# (which need not form valid characters) and shall not contain any null bytes.
# REQUIREMENT: SHALL-READ-1288:
# If the -d delim option is specified and delim is the null string, the
# standard input shall contain zero or more bytes (which need not form valid
# characters).
# REQUIREMENT: SHALL-SH-1017:
# The following environment variables shall affect the execution of sh : ENV
# This variable, when and only when an interactive shell is invoked, shall be
# subjected to parameter expansion (see 2.6.2 Parameter Expansion ) by the
# shell, and the resulting value shall be used as a pathname of a file
# containing shell commands to execute in the current environment.
# REQUIREMENT: SHALL-READ-1290:
# PS2 Provide the prompt string that an interactive shell shall write to
# standard error when a line ending with a <backslash> <newline> is read and the
# -r option was not specified.
# REQUIREMENT: SHALL-READ-1291:
# The standard error shall be used for diagnostic messages and prompts for
# continued input.
# REQUIREMENT: SHALL-SH-1024-DUP746:
# The following exit values shall be returned: 0 The script to be executed
# consisted solely of zero or more blank lines or comments, or both.

test_cmd='
    echo "word1 word2" | read v1 v2
    echo "$v1/$v2"
'
# POSIX allows read in pipeline to be in subshell, meaning variables aren't
# preserved in parent.
# Let's use a here-doc or redirection.
test_cmd='
    read v1 v2 <<INEOF
word1 word2
INEOF
    echo "$v1/$v2"
'
assert_stdout "word1/word2" "$TARGET_SHELL -c '$test_cmd'"

# test -r option
test_cmd='
    read -r v1 <<\INEOF
word1\
word2
INEOF
    echo "$v1"
'
# With -r, backslash is literal
assert_stdout 'word1\' "$TARGET_SHELL -c '$test_cmd'"

# test without -r (backslash escapes newline)
test_cmd='
    read v1 <<INEOF
word1\
word2
INEOF
    echo "$v1"
'
assert_stdout "word1word2" "$TARGET_SHELL -c '$test_cmd'"


# REQUIREMENT: SHALL-GETOPTS-1175:
# It shall support the Utility Syntax Guidelines 3 to 10, inclusive, described
# in XBD 12.2 Utility Syntax Guidelines .
# REQUIREMENT: SHALL-GETOPTS-1176:
# When the shell is first invoked, the shell variable OPTIND shall be
# initialized to 1.
# REQUIREMENT: SHALL-GETOPTS-1178:
# Each time getopts is invoked, it shall place the value of the next option
# found in the parameter list in the shell variable specified by the name
# operand and the shell variable OPTIND shall be set as follows: When getopts
# successfully parses an option that takes an option-argument (that is, a
# character followed by <colon> in optstring , and exit status is 0), the value
# of OPTIND shall be the integer index of the next element of the parameter list
# (if any; see OPERANDS below) to be searched for an option character.
# REQUIREMENT: SHALL-GETOPTS-1178:
# Each time getopts is invoked, it shall place the value of the next option
# found in the parameter list in the shell variable specified by the name
# operand and the shell variable OPTIND shall be set as follows: When getopts
# successfully parses an option that takes an option-argument (that is, a
# character followed by <colon> in optstring , and exit status is 0), the value
# of OPTIND shall be the integer index of the next element of the parameter list
# (if any; see OPERANDS below) to be searched for an option character.
# REQUIREMENT: SHALL-GETOPTS-1179:
# When getopts reports end of options (that is, when exit status is 1), the
# value of OPTIND shall be the integer index of the next element of the
# parameter list (if any).
# REQUIREMENT: SHALL-GETOPTS-1180:
# In all other cases, the value of OPTIND is unspecified, but shall encode the
# information needed for the next invocation of getopts to resume parsing
# options after the option just parsed.
# REQUIREMENT: SHALL-GETOPTS-1181:
# When the option requires an option-argument, the getopts utility shall place
# it in the shell variable OPTARG .
# REQUIREMENT: SHALL-GETOPTS-1182:
# If no option was found, or if the option that was found does not have an
# option-argument, OPTARG shall be unset.
# REQUIREMENT: SHALL-GETOPTS-1183:
# If an option character not contained in the optstring operand is found where
# an option character is expected, the shell variable specified by name shall be
# set to the <question-mark> ( '?' ) character.
# REQUIREMENT: SHALL-GETOPTS-1184:
# In this case, if the first character in optstring is a <colon> ( ':' ), the
# shell variable OPTARG shall be set to the option character found, but no
# output shall be written to standard error; otherwise, the shell variable
# OPTARG shall be unset and a diagnostic message shall be written to standard
# error.
# REQUIREMENT: SHALL-GETOPTS-1185:
# This condition shall be considered to be an error detected in the way
# arguments were presented to the invoking application, but shall not be an
# error in getopts processing.

test_cmd='
    set -- -a -b foo bar
    getopts "ab:" opt
    echo "$opt"
    getopts "ab:" opt
    echo "$opt $OPTARG"
    getopts "ab:" opt
    echo "$?"
    echo "$OPTIND"
'
assert_stdout "a
b foo
1
4" "$TARGET_SHELL -c '$test_cmd'"

# Error handling: missing option argument (silent mode with colon)
test_cmd='
    set -- -x -b
    getopts ":ab:" opt
    echo "$opt $OPTARG"
    getopts ":ab:" opt
    echo "$opt $OPTARG"
'
# In silent mode:
# -x is unknown -> opt=? OPTARG=x
# -b is missing arg -> opt=: OPTARG=b
assert_stdout "? x
: b" "$TARGET_SHELL -c '$test_cmd'"


# Error handling: missing option argument (verbose mode without colon)
test_cmd='
    set -- -b
    getopts "ab:" opt 2>/dev/null
    echo "$opt ${OPTARG:-unset}"
'
# In verbose mode:
# -b is missing arg -> opt=? OPTARG is unset
assert_stdout "? unset" "$TARGET_SHELL -c '$test_cmd'"

report
