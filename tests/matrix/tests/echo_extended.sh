# Test: echo — Write Arguments to Standard Output
# Target: tests/matrix/tests/echo_extended.sh
#
# POSIX compliance tests for the echo utility covering operand handling,
# XSI escape sequences, output format, and option restrictions.

. "$MATRIX_DIR/lib.sh"

# ==============================================================================
# echo shall not recognize "--" argument
# ==============================================================================
# REQUIREMENT: SHALL-OPTIONS-5000:
# echo shall not recognize the "--" argument in that context; it shall be
# treated as a string operand.

assert_stdout "--" "$TARGET_SHELL -c 'echo --'"

assert_stdout "-- hello" "$TARGET_SHELL -c 'echo -- hello'"

assert_stdout "-- --" "$TARGET_SHELL -c 'echo -- --'"

# ==============================================================================
# String operand shall be supported
# ==============================================================================
# REQUIREMENT: SHALL-OPERANDS-5001:
# string operand shall be supported.

assert_stdout "hello" "$TARGET_SHELL -c 'echo hello'"

assert_stdout "" "$TARGET_SHELL -c 'echo'"

assert_stdout "hello world" "$TARGET_SHELL -c 'echo hello world'"

assert_stdout "one two three" "$TARGET_SHELL -c 'echo one two three'"

# Single character operand.
assert_stdout "x" "$TARGET_SHELL -c 'echo x'"

# ==============================================================================
# If first operand is - followed by e, E, n chars, behavior unspecified
# ==============================================================================
# REQUIREMENT: SHALL-OPERANDS-5002:
# If the first operand is -n, -e, -E, or any combination of those characters
# following a hyphen, behavior is implementation-defined (unspecified).
# We just verify echo does not crash and exits 0.

assert_exit_code 0 "$TARGET_SHELL -c 'echo -n test >/dev/null'"
assert_exit_code 0 "$TARGET_SHELL -c 'echo -e test >/dev/null'"
assert_exit_code 0 "$TARGET_SHELL -c 'echo -E test >/dev/null'"
assert_exit_code 0 "$TARGET_SHELL -c 'echo -nee test >/dev/null'"
assert_exit_code 0 "$TARGET_SHELL -c 'echo -neE test >/dev/null'"

# ==============================================================================
# XSI escape sequences
# ==============================================================================
# REQUIREMENT: SHALL-OPERANDS-5003:
# XSI escape sequences: \a, \b, \c, \f, \n, \r, \t, \v, \\, \0num

# \a — alert (bell)
_out=$($TARGET_SHELL -c 'echo "\a"' 2>/dev/null | od -An -tx1 | tr -d ' \n')
case "$_out" in
    *07*) pass ;;
    *) fail "echo \\a did not produce BEL (0x07), got: $_out" ;;
esac

# \b — backspace
_out=$($TARGET_SHELL -c 'echo "\b"' 2>/dev/null | od -An -tx1 | tr -d ' \n')
case "$_out" in
    *08*) pass ;;
    *) fail "echo \\b did not produce BS (0x08), got: $_out" ;;
esac

# \f — form feed
_out=$($TARGET_SHELL -c 'echo "\f"' 2>/dev/null | od -An -tx1 | tr -d ' \n')
case "$_out" in
    *0c*) pass ;;
    *) fail "echo \\f did not produce FF (0x0c), got: $_out" ;;
esac

# \n — newline (extra newline in output)
_out=$($TARGET_SHELL -c 'echo "\n"' 2>/dev/null | od -An -tx1 | tr -d ' \n')
# Expect at least two 0a bytes: the \n escape plus trailing newline.
case "$_out" in
    *0a0a*) pass ;;
    *) fail "echo \\n did not produce two newlines, got: $_out" ;;
esac

# \r — carriage return
_out=$($TARGET_SHELL -c 'echo "\r"' 2>/dev/null | od -An -tx1 | tr -d ' \n')
case "$_out" in
    *0d*) pass ;;
    *) fail "echo \\r did not produce CR (0x0d), got: $_out" ;;
esac

# \t — horizontal tab
_out=$($TARGET_SHELL -c 'echo "\t"' 2>/dev/null | od -An -tx1 | tr -d ' \n')
case "$_out" in
    *09*) pass ;;
    *) fail "echo \\t did not produce HT (0x09), got: $_out" ;;
esac

# \v — vertical tab
_out=$($TARGET_SHELL -c 'echo "\v"' 2>/dev/null | od -An -tx1 | tr -d ' \n')
case "$_out" in
    *0b*) pass ;;
    *) fail "echo \\v did not produce VT (0x0b), got: $_out" ;;
esac

# \\ — literal backslash
assert_stdout '\' "$TARGET_SHELL -c 'echo \"\\\\\"'"

# \0num — octal value
# \065 is ASCII '5'
_out=$($TARGET_SHELL -c 'echo "\065"' 2>/dev/null)
case "$_out" in
    *5*) pass ;;
    *) fail "echo \\065 did not produce '5', got: $_out" ;;
esac

# \0101 is ASCII 'A'
_out=$($TARGET_SHELL -c 'echo "\0101"' 2>/dev/null)
case "$_out" in
    *A*) pass ;;
    *) fail "echo \\0101 did not produce 'A', got: $_out" ;;
esac

# ==============================================================================
# All characters following \c shall be ignored
# ==============================================================================
# REQUIREMENT: SHALL-OPERANDS-5004:
# All characters following \c in the string shall be ignored. The trailing
# newline is also suppressed.

# Text before \c is printed; text after \c (and trailing newline) is suppressed.
_out=$($TARGET_SHELL -c 'echo "hello\c world"' 2>/dev/null | od -An -tx1 | tr -d ' \n')
# Should contain 68656c6c6f (hello) and NOT 0a at end, NOT contain "world".
case "$_out" in
    *776f726c64*) fail "echo \\c did not suppress text after \\c, got: $_out" ;;
    *68656c6c6f*) pass ;;
    *) fail "echo \\c did not output text before \\c correctly, got: $_out" ;;
esac

# Verify no trailing newline after \c.
_bytes=$($TARGET_SHELL -c 'echo "AB\c"' 2>/dev/null | wc -c | tr -d ' ')
if [ "$_bytes" = "2" ]; then
    pass
else
    fail "echo with \\c should produce exactly 2 bytes (AB, no newline), got $_bytes"
fi

# ==============================================================================
# Arguments separated by single space, followed by newline
# ==============================================================================
# REQUIREMENT: SHALL-STDOUT-5006:
# Arguments shall be separated by single <space> characters and followed
# by a <newline> character.

# Multiple arguments joined by single spaces.
assert_stdout "a b c" "$TARGET_SHELL -c 'echo a b c'"

# Verify trailing newline exists.
_bytes=$($TARGET_SHELL -c 'echo hello' 2>/dev/null | wc -c | tr -d ' ')
if [ "$_bytes" = "6" ]; then
    pass
else
    fail "echo hello should produce 6 bytes (hello + newline), got $_bytes"
fi

# With no arguments, only a newline is produced.
_bytes=$($TARGET_SHELL -c 'echo' 2>/dev/null | wc -c | tr -d ' ')
if [ "$_bytes" = "1" ]; then
    pass
else
    fail "echo with no args should produce 1 byte (newline), got $_bytes"
fi

# Many arguments.
assert_stdout "1 2 3 4 5" "$TARGET_SHELL -c 'echo 1 2 3 4 5'"

# ==============================================================================
# Implementations shall not support any options
# ==============================================================================
# REQUIREMENT: SHALL-ISSUE-6-5007:
# Implementations shall not support any options. All arguments, including
# those that look like options, are treated as string operands.

# A double-dash flag-like argument is not treated as end-of-options.
assert_stdout "-- hello" "$TARGET_SHELL -c 'echo -- hello'"

# Unknown flags are treated as operands (not parsed as options).
assert_stdout "-x" "$TARGET_SHELL -c 'echo -x'"
assert_stdout "-abc" "$TARGET_SHELL -c 'echo -abc'"
assert_stdout "--help" "$TARGET_SHELL -c 'echo --help'"
assert_stdout "--version" "$TARGET_SHELL -c 'echo --version'"

report
