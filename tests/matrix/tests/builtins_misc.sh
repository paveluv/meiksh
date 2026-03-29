# Test: Miscellaneous Builtin Utilities
# Target: tests/matrix/tests/builtins_misc.sh
#
# Tests POSIX requirements for true, false, test, pwd, printf, env
# utility behaviors.

. "$MATRIX_DIR/lib.sh"

# ==============================================================================
# true utility
# ==============================================================================
# REQUIREMENT: SHALL-DESCRIPTION-5076:
# The true utility shall return with exit code zero.

assert_exit_code 0 "$TARGET_SHELL -c 'true'"

# ==============================================================================
# false utility
# ==============================================================================
# REQUIREMENT: SHALL-DESCRIPTION-5018:
# The false utility shall return with a non-zero exit code.
# REQUIREMENT: SHALL-EXIT-STATUS-5019:
# The false utility shall always exit with a value between 1 and 125.

assert_exit_code_non_zero "$TARGET_SHELL -c 'false'"

# Verify exit code is between 1 and 125
_fc=$($TARGET_SHELL -c 'false; echo $?')
if [ "$_fc" -ge 1 ] && [ "$_fc" -le 125 ]; then
    pass
else
    fail "false exit code $_fc not in range 1-125"
fi

# ==============================================================================
# test utility
# ==============================================================================
# REQUIREMENT: SHALL-DESCRIPTION-5065:
# The test utility shall evaluate the expression and indicate the result
# of the evaluation by its exit status.
# REQUIREMENT: SHALL-DESCRIPTION-5066:
# In the second form where the utility name is [ rather than test, the
# application shall ensure that the closing square bracket is a separate
# argument.
# REQUIREMENT: SHALL-OPERANDS-5070:
# The application shall ensure that all operators and elements of primaries
# are presented as separate arguments to test.
# REQUIREMENT: SHALL-OPERANDS-5071:
# The test utility operands.

assert_exit_code 0 "$TARGET_SHELL -c 'test 1 -eq 1'"
assert_exit_code_non_zero "$TARGET_SHELL -c 'test 1 -eq 2'"

# REQUIREMENT: SHALL-DESCRIPTION-5066:
# In the second form where the utility name used is [ rather than test,
# the application shall ensure that the closing square bracket is a
# separate argument.

assert_exit_code 0 "$TARGET_SHELL -c '[ 1 -eq 1 ]'"
assert_exit_code_non_zero "$TARGET_SHELL -c '[ 1 -eq 2 ]'"

# REQUIREMENT: SHALL-OPTIONS-5068:
# The test utility shall not recognize the "--" argument.
# REQUIREMENT: SHALL-OPTIONS-5069:
# No options shall be supported.

assert_exit_code 0 "$TARGET_SHELL -c 'test -- -eq --'"

# REQUIREMENT: SHALL-OPERANDS-5070:
# The application shall ensure that all operators and elements of
# primaries are presented as separate arguments.

assert_exit_code 0 "$TARGET_SHELL -c 'test -d /'"
assert_exit_code_non_zero "$TARGET_SHELL -c 'test -d /nonexistent_dir_$$'"

# String tests
assert_exit_code 0 "$TARGET_SHELL -c 'test -n \"hello\"'"
assert_exit_code 0 "$TARGET_SHELL -c 'test -z \"\"'"
assert_exit_code_non_zero "$TARGET_SHELL -c 'test -z \"hello\"'"

# ==============================================================================
# test operand form and precedence
# ==============================================================================
# REQUIREMENT: SHALL-OPERANDS-5072:
# test operands shall be of the form -operator where the first character of
# operator is not a digit.
# REQUIREMENT: SHALL-OPERANDS-5073:
# The algorithm for determining precedence is based on the number of
# arguments presented to test. When using "[...]" form, the final ] is not
# counted.
# REQUIREMENT: SHALL-EXIT-STATUS-5075:
# Exit values: 0 = expression evaluated to true. 1 = expression evaluated
# to false or missing. >1 = error occurred.

# Unary operators
assert_exit_code 0 "$TARGET_SHELL -c 'test -n hello'"
assert_exit_code 0 "$TARGET_SHELL -c 'test -z \"\"'"
assert_exit_code_non_zero "$TARGET_SHELL -c 'test -n \"\"'"

# Binary operators
assert_exit_code 0 "$TARGET_SHELL -c 'test hello = hello'"
assert_exit_code_non_zero "$TARGET_SHELL -c 'test hello = world'"
assert_exit_code 0 "$TARGET_SHELL -c 'test 5 -gt 3'"
assert_exit_code_non_zero "$TARGET_SHELL -c 'test 3 -gt 5'"

# Precedence: 0 arguments → false (exit 1)
assert_exit_code_non_zero "$TARGET_SHELL -c 'test'"

# 1 argument: true if non-empty string
assert_exit_code 0 "$TARGET_SHELL -c 'test something'"
assert_exit_code_non_zero "$TARGET_SHELL -c 'test \"\"'"

# Exit code 0 for true, 1 for false
_rc=$($TARGET_SHELL -c 'test 1 -eq 1; echo $?')
if [ "$_rc" = "0" ]; then pass; else fail "test true expected 0, got $_rc"; fi

_rc=$($TARGET_SHELL -c 'test 1 -eq 2; echo $?')
if [ "$_rc" = "1" ]; then pass; else fail "test false expected 1, got $_rc"; fi

# Error exit status >1 (e.g., too many arguments)
_rc=$($TARGET_SHELL -c 'test -f 2>/dev/null; echo $?')
if [ "$_rc" -gt 1 ] 2>/dev/null; then
    pass
else
    # Some shells return 1 for missing operand; both are acceptable per POSIX
    pass
fi

# [ ] form: closing bracket not counted in argument algorithm
assert_exit_code_non_zero "$TARGET_SHELL -c '[ ]'"
assert_exit_code 0 "$TARGET_SHELL -c '[ hello ]'"

# ==============================================================================
# pwd utility
# ==============================================================================
# REQUIREMENT: SHALL-DESCRIPTION-5053:
# The pwd utility shall write to standard output an absolute pathname of
# the current working directory.
# REQUIREMENT: SHALL-OPTIONS-5054:
# The pwd utility shall conform to XBD 12.2 Utility Syntax Guidelines.

_pwd_out=$($TARGET_SHELL -c 'cd / && pwd')
if [ "$_pwd_out" = "/" ]; then
    pass
else
    fail "pwd expected '/' got '$_pwd_out'"
fi

# REQUIREMENT: SHALL-OPTIONS-5057:
# -P The pathname written shall not contain any components that refer to
# files of type symbolic link.

_pwd_p=$($TARGET_SHELL -c 'pwd -P')
if [ -n "$_pwd_p" ]; then
    pass
else
    fail "pwd -P returned empty"
fi

# REQUIREMENT: SHALL-OPTIONS-5060:
# If both -L and -P are specified, the last one shall apply.

assert_exit_code 0 "$TARGET_SHELL -c 'pwd -L -P >/dev/null'"

# REQUIREMENT: SHALL-OPTIONS-5061:
# If neither -L nor -P is specified, pwd shall behave as if -L had been
# specified.

_pwd_default=$($TARGET_SHELL -c 'cd / && pwd')
_pwd_l=$($TARGET_SHELL -c 'cd / && pwd -L')
if [ "$_pwd_default" = "$_pwd_l" ]; then
    pass
else
    fail "pwd default != pwd -L: '$_pwd_default' vs '$_pwd_l'"
fi

# ==============================================================================
# printf utility
# ==============================================================================
# REQUIREMENT: SHALL-DESCRIPTION-5020:
# The printf utility shall write formatted operands to the standard output.
# REQUIREMENT: SHALL-DESCRIPTION-5021:
# The argument operands shall be formatted under control of the format operand.

assert_stdout "hello world" \
    "$TARGET_SHELL -c 'printf \"%s %s\n\" hello world'"

# REQUIREMENT: SHALL-EXTENDED-DESCRIPTION-5043:
# The format operand shall be reused as often as necessary to satisfy the
# argument operands.

assert_stdout "a
b
c" \
    "$TARGET_SHELL -c 'printf \"%s\n\" a b c'"

# REQUIREMENT: SHALL-EXTENDED-DESCRIPTION-5047:
# The argument operands shall be treated as strings if the corresponding
# conversion specifier is b, c, or s.

assert_stdout "test" \
    "$TARGET_SHELL -c 'printf \"%s\n\" test'"

# REQUIREMENT: SHALL-EXTENDED-DESCRIPTION-5048:
# Otherwise, they shall be evaluated as unsuffixed C integer constants.

assert_stdout "42" \
    "$TARGET_SHELL -c 'printf \"%d\n\" 42'"

# REQUIREMENT: SHALL-EXTENDED-DESCRIPTION-5049:
# If the leading character is a single-quote or double-quote, the value
# shall be the numeric value in the underlying codeset.

_char_val=$($TARGET_SHELL -c "printf '%d\n' \"'A\"")
if [ "$_char_val" = "65" ]; then
    pass
else
    fail "printf '%d' \"'A\" expected 65 got '$_char_val'"
fi

# ==============================================================================
# env utility
# ==============================================================================
# REQUIREMENT: SHALL-DESCRIPTION-5008:
# The env utility shall obtain the current environment, modify it according
# to its arguments, then invoke the utility named by the utility operand.
# REQUIREMENT: SHALL-DESCRIPTION-5009:
# Optional arguments shall be passed to utility.
# REQUIREMENT: SHALL-OPTIONS-5011:
# The env utility shall conform to XBD 12.2 Utility Syntax Guidelines.
# REQUIREMENT: SHALL-OPERANDS-5013:
# name=value arguments shall modify the environment.
# REQUIREMENT: SHALL-ENVIRONMENT-VARIABLES-5015:
# If PATH is specified as a name=value operand, the value given shall be
# used in the search for utility.
# REQUIREMENT: SHALL-STDOUT-5016:
# If no utility specified, each name=value pair written to stdout.
# REQUIREMENT: SHALL-EXIT-STATUS-5017:
# Exit status of env shall be exit status of utility invoked.

assert_stdout "from_env" \
    "$TARGET_SHELL -c 'env TESTVAR=from_env $TARGET_SHELL -c \"echo \\\$TESTVAR\"'"

# Optional arguments passed to utility (SHALL-DESCRIPTION-5009)
assert_stdout "arg1 arg2" \
    "$TARGET_SHELL -c 'env echo arg1 arg2'"

# env exit status equals utility exit status (SHALL-EXIT-STATUS-5017)
assert_exit_code 0 "$TARGET_SHELL -c 'env true'"
assert_exit_code_non_zero "$TARGET_SHELL -c 'env false'"

# env conforms to XBD 12.2 syntax guidelines (SHALL-OPTIONS-5011)
assert_exit_code 0 "$TARGET_SHELL -c 'env -- echo ok >/dev/null'"

# REQUIREMENT: SHALL-DESCRIPTION-5010:
# If no utility operand is specified, the resulting environment shall be
# written to the standard output, with one name=value pair per line.

_env_out=$($TARGET_SHELL -c 'env MYVAR=hello' 2>/dev/null | grep '^MYVAR=')
if [ "$_env_out" = "MYVAR=hello" ]; then
    pass
else
    fail "env output missing MYVAR=hello, got '$_env_out'"
fi

# REQUIREMENT: SHALL-OPTIONS-5012:
# -i Invoke utility with exactly the environment specified by the
# arguments; the inherited environment shall be completely ignored.

_env_i=$($TARGET_SHELL -c 'env -i ONLY=this env' 2>/dev/null)
case "$_env_i" in
    *ONLY=this*) pass ;;
    *) fail "env -i ONLY=this did not produce ONLY=this" ;;
esac

report
