# Test: Shell Invocation Options and Operands
# Target: tests/matrix/tests/sh_options.sh
#
# POSIX specifies how the `sh` utility itself parses options, handles
# the `-c` and `-s` flags, and assigns positional parameters based on
# operands.

. "$MATRIX_DIR/lib.sh"


# ==============================================================================
# The '-c' Option
# ==============================================================================
# REQUIREMENT: SHALL-OPTIONS-004:
# The sh utility shall conform to XBD 12.2 Utility Syntax Guidelines , with an
# extension for support of a leading <plus-sign> ( '+' ) as noted below.
# REQUIREMENT: SHALL-SH-1021:
# The following additional options shall be supported: -c Read commands from
# the command_string operand.
# REQUIREMENT: SHALL-OPTIONS-007:
# No commands shall be read from the standard input.
# REQUIREMENT: SHALL-OPERANDS-013:
# Special parameter 0 (see 2.5.2 Special Parameters ) shall be set to the value
# of command_file .
# REQUIREMENT: SHALL-OPERANDS-011:
# argument The positional parameters ($1, $2, and so on) shall be set to
# arguments , if any.

# When `-c` is used, the next argument is the command. The argument after that
# is `$0`, followed by `$1`, `$2`, etc.
test_cmd='echo "args: $0 $1 $2"'
assert_stdout "args: zero one two" \
    "$TARGET_SHELL -c '$test_cmd' zero one two"


# ==============================================================================
# The '-s' Option and Standard Input
# ==============================================================================
# REQUIREMENT: SHALL-OPERANDS-009:
# The following operands shall be supported:...
# REQUIREMENT: SHALL-OPTIONS-008:
# If there are no operands and the -c option is not specified, the -s option
# shall be assumed.
# REQUIREMENT: SHALL-STDIN-015:
# The standard input shall be used only if one of
# the following is true:...
# REQUIREMENT: SHALL-STDIN-016:
# When the shell is using standard input and it invokes a command that also
# uses standard input, the shell shall ensure that the standard input file
# pointer points directly after the command it has read when the command begins
# execution.
# REQUIREMENT: SHALL-STDIN-017:
# It shall not read ahead in such a manner that any characters intended to be
# read by the invoked command are consumed by the shell (whether interpreted by
# the shell or not) or that characters that are not read by the invoked command
# are not seen by the shell.
# REQUIREMENT: SHALL-STDIN-018:
# If the standard input to sh is a FIFO or terminal device and is set to
# non-blocking reads, then sh shall enable blocking reads on standard input.
# REQUIREMENT: SHALL-STDIN-019:
# This shall remain in effect when the command completes.

# If we feed commands via stdin, it acts like `-s`.
assert_stdout "stdin_test" \
    "echo 'echo stdin_test' | $TARGET_SHELL"

# Testing that standard input is not consumed entirely if a command needs it.
# We create a script that reads one line and echos it. The shell should read the
# read command, then the read command gets the second line, not the shell.
test_cmd='
read line
echo "got: $line"
'
assert_stdout "got: input_for_read" \
    "(echo 'read line'; echo 'input_for_read'; echo 'echo \"got: \$line\"') | $TARGET_SHELL"

# REQUIREMENT: SHALL-OPERANDS-014:
# If sh is called using a synopsis form that omits command_file , special
# parameter 0 shall be set to the value of the first argument passed to sh from
# its parent (for example, argv [0] for a C program), which is normally a
# pathname used to execute the sh utility.

assert_stdout "$TARGET_SHELL" \
    "echo 'echo \$0' | $TARGET_SHELL"

# ==============================================================================
# The Hyphen Operand
# ==============================================================================
# REQUIREMENT: SHALL-OPERANDS-010:
# The following operands shall be supported: - A single <hyphen-minus> shall be
# treated as the first operand and then ignored.

# `sh -` just ignores the hyphen and reads from stdin.
assert_stdout "hyphen_test" \
    "echo 'echo hyphen_test' | $TARGET_SHELL -"

# ==============================================================================
# Option Parsing (+ / -)
# ==============================================================================
# REQUIREMENT: SHALL-OPTIONS-005:
# The option letters derived from the set special built-in shall also be
# accepted with a leading <plus-sign> ( '+' ) instead of a leading
# <hyphen-minus> (meaning the reverse case of the option as described in this
# volume of POSIX.1-2024).

# We can turn off `allexport` with `+a`.
# Let's turn ON `set -a`, define a var, and see it exported. Then turn OFF `set
# +a`.
test_cmd='
set -a
export_on="yes"
set +a
export_off="no"
env | grep export_
'
assert_stdout "export_on=yes" \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# The 'sh' Invocation File Search
# ==============================================================================
# REQUIREMENT: SHALL-OPERANDS-012:
# If the pathname does not contain a <slash> character: The implementation
# shall attempt to read that file from the current working directory; the file
# need not be executable.

echo 'echo "local_script_executed"' > tmp_local.sh
assert_stdout "local_script_executed" \
    "$TARGET_SHELL tmp_local.sh"

report
