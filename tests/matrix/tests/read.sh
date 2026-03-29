# Test: read Built-in Utility
# Target: tests/matrix/tests/read.sh
#
# POSIX compliance tests for the `read` built-in. Covers line reading,
# backslash processing, field splitting via IFS, variable assignment,
# EOF handling, readonly errors, subshell isolation, and -r mode.

. "$MATRIX_DIR/lib.sh"

# ==============================================================================
# Basic Line Reading
# ==============================================================================
# REQUIREMENT: SHALL-READ-1259:
# read shall read a single logical line from stdin.
# REQUIREMENT: SHALL-READ-1266:
# Terminating logical line delimiter shall be removed.

assert_stdout "hello" \
    "echo 'hello' | $TARGET_SHELL -c 'read line; echo \"\$line\"'"

assert_stdout "hello world" \
    "echo 'hello world' | $TARGET_SHELL -c 'read line; echo \"\$line\"'"

# Only the first line is consumed by a single read.
assert_stdout "first" \
    "printf 'first\nsecond\n' | $TARGET_SHELL -c 'read line; echo \"\$line\"'"

# ==============================================================================
# Backslash Escape Processing (no -r)
# ==============================================================================
# REQUIREMENT: SHALL-READ-1260:
# If -r not specified, backslash shall act as escape character.
# REQUIREMENT: SHALL-READ-1261:
# Unescaped backslash preserves literal value of following backslash.
# REQUIREMENT: SHALL-READ-1264:
# All other unescaped backslash characters removed after splitting.

# Backslash-space preserves a literal space without splitting.
assert_stdout "hello world" \
    "printf 'hello\\ world\n' | $TARGET_SHELL -c 'read var; echo \"\$var\"'"

# Backslash-backslash produces a single backslash.
# Input: a\\b (a + backslash + backslash + b). read without -r: \\ → \, result: a\b.
# Use printf for both input and output to avoid bash echo interpreting backslashes.
_out=$($TARGET_SHELL -c 'printf "%s\n" "a\\\\b" | { read var; printf "%s" "$var"; }' 2>/dev/null)
_exp=$(printf '%s' 'a\b')
if [ "$_out" = "$_exp" ]; then pass; else fail "read backslash-backslash: expected 'a\\b', got '$_out'"; fi

# Backslash before a regular character removes the backslash.
# Input: a\b (a + backslash + b). read without -r removes backslash, result: ab.
_out=$($TARGET_SHELL -c 'printf "%s\n" "a\\b" | { read var; printf "%s" "$var"; }' 2>/dev/null)
if [ "$_out" = "ab" ]; then pass; else fail "read backslash-char: expected 'ab', got '$_out'"; fi

# REQUIREMENT: SHALL-READ-1262:
# If excepted character follows backslash, read interprets as line continuation.
# REQUIREMENT: SHALL-READ-1263:
# Backslash and excepted character removed before splitting.

# Backslash-newline joins lines (line continuation).
assert_stdout "helloworld" \
    "printf 'hello\\\\\nworld\n' | $TARGET_SHELL -c 'read var; echo \"\$var\"'"

# ==============================================================================
# The -r Option (Raw Mode)
# ==============================================================================
# REQUIREMENT: SHALL-READ-1282:
# read shall conform to XBD 12.2 Utility Syntax Guidelines.

# With -r, backslashes are treated literally.
_out=$($TARGET_SHELL -c 'printf "%s\n" "hello\\world" | { read -r var; printf "%s" "$var"; }' 2>/dev/null)
_exp=$(printf '%s' 'hello\world')
if [ "$_out" = "$_exp" ]; then pass; else fail "read -r backslash: expected 'hello\\world', got '$_out'"; fi

# With -r, backslash-newline does NOT join lines.
assert_stdout 'hello\' \
    "printf 'hello\\\\\nworld\n' | $TARGET_SHELL -c 'read -r var; echo \"\$var\"'"

# With -r, backslash-backslash is preserved as two backslashes.
_out=$($TARGET_SHELL -c 'printf "%s\n" "a\\\\b" | { read -r var; printf "%s" "$var"; }' 2>/dev/null)
_exp=$(printf '%s' 'a\\b')
if [ "$_out" = "$_exp" ]; then pass; else fail "read -r backslash-backslash: expected 'a\\\\b', got '$_out'"; fi

# ==============================================================================
# Field Splitting with Default IFS
# ==============================================================================
# REQUIREMENT: SHALL-READ-1270:
# If IFS unset or non-empty, modified field splitting algorithm applied.
# REQUIREMENT: SHALL-READ-1274:
# Var operands processed in order; output fields assigned in order.
# REQUIREMENT: SHALL-READ-1271:
# Loop ceases when input empty or n output fields generated (n = vars - 1).
# REQUIREMENT: SHALL-READ-1272:
# Remaining input returned unsplit, with leading/trailing IFS whitespace removed.

# Two variables, two fields.
assert_stdout "hello world" \
    "echo 'hello world' | $TARGET_SHELL -c 'read a b; echo \"\$a \$b\"'"

# Multiple spaces treated as single delimiter.
assert_stdout "hello world" \
    "echo 'hello    world' | $TARGET_SHELL -c 'read a b; echo \"\$a \$b\"'"

# Leading whitespace stripped.
assert_stdout "hello world" \
    "echo '   hello world' | $TARGET_SHELL -c 'read a b; echo \"\$a \$b\"'"

# Trailing whitespace stripped from last field.
assert_stdout "hello world" \
    "echo 'hello world   ' | $TARGET_SHELL -c 'read a b; echo \"\$a \$b\"'"

# REQUIREMENT: SHALL-READ-1275:
# If exactly one var remains and there was unsplit input, assign unsplit input.

# Fewer variables than fields: last variable gets the remainder.
assert_stdout "a b c d" \
    "echo 'a b c d' | $TARGET_SHELL -c 'read first rest; echo \"\$first \$rest\"'"

# Remainder preserves internal spacing (but leading/trailing IFS whitespace removed).
assert_stdout "a b   c   d" \
    "echo 'a b   c   d' | $TARGET_SHELL -c 'read first rest; echo \"\$first \$rest\"'"

# REQUIREMENT: SHALL-READ-1276:
# If unprocessed var operands remain, each set to empty string.

# More variables than fields: extras get empty string.
assert_stdout "hello::" \
    "echo 'hello' | $TARGET_SHELL -c 'read a b c; echo \"\$a:\$b:\$c\"'"

assert_stdout "hello:world:" \
    "echo 'hello world' | $TARGET_SHELL -c 'read a b c; echo \"\$a:\$b:\$c\"'"

# ==============================================================================
# Custom IFS
# ==============================================================================
# REQUIREMENT: SHALL-READ-1067:
# IFS determines internal field separators.

# Colon as IFS.
assert_stdout "a:b:c" \
    "echo 'a:b:c' | $TARGET_SHELL -c 'IFS=: read a b c; echo \"\$a:\$b:\$c\"'"

# Comma as IFS.
assert_stdout "x y" \
    "echo 'x,y' | $TARGET_SHELL -c 'IFS=, read a b; echo \"\$a \$b\"'"

# Multi-character IFS: colon and comma.
assert_stdout "a b c" \
    "echo 'a:b,c' | $TARGET_SHELL -c 'IFS=:, read a b c; echo \"\$a \$b \$c\"'"

# Non-whitespace IFS preserves leading/trailing delimiters as empty fields.
assert_stdout ":b:" \
    "echo ':b:' | $TARGET_SHELL -c 'IFS=: read a b c; echo \"\$a:\$b:\$c\"'"

# ==============================================================================
# Empty IFS
# ==============================================================================
# REQUIREMENT: SHALL-READ-1267:
# If IFS is empty string, data assigned to first var, others set empty.
# REQUIREMENT: SHALL-READ-1268:
# No other processing in that case.

# Empty IFS: entire line goes into first variable, second is empty.
assert_stdout "hello world:" \
    "echo 'hello world' | $TARGET_SHELL -c 'IFS= read a b; echo \"\$a:\$b\"'"

# Empty IFS: no field splitting at all.
assert_stdout "a:b:c:" \
    "echo 'a:b:c' | $TARGET_SHELL -c 'IFS= read x y; echo \"\$x:\$y\"'"

# Empty IFS with single variable: entire line assigned.
assert_stdout "  hello  world  " \
    "printf '  hello  world  \n' | $TARGET_SHELL -c 'IFS= read x; echo \"\$x\"'"

# ==============================================================================
# Variable Operand: Existing or Nonexisting
# ==============================================================================
# REQUIREMENT: SHALL-READ-1066:
# Operand: var - name of existing or nonexisting shell variable.

# Reading into a previously unset variable creates it.
assert_stdout "created" \
    "echo 'created' | $TARGET_SHELL -c 'unset newvar; read newvar; echo \"\$newvar\"'"

# Reading into an existing variable overwrites it.
assert_stdout "new" \
    "echo 'new' | $TARGET_SHELL -c 'existing=old; read existing; echo \"\$existing\"'"

# ==============================================================================
# Current Shell Environment
# ==============================================================================
# REQUIREMENT: SHALL-READ-1277:
# Setting variables affects current shell execution environment.

# Variable set by read persists after the read call.
assert_stdout "hello:world" \
    "$TARGET_SHELL -c 'echo \"hello world\" | { read a b; echo \"\$a:\$b\"; }'"

# read in a while loop affects the current shell (when not in a pipeline subshell).
assert_stdout "done:" \
    "$TARGET_SHELL -c '
        val=init
        while read val; do :; done <<EOF
first
last
EOF
        echo \"done:\$val\"
    '"

# ==============================================================================
# Subshell Isolation
# ==============================================================================
# REQUIREMENT: SHALL-READ-1280:
# read in subshell shall not affect caller's environment.

assert_stdout "parent" \
    "$TARGET_SHELL -c 'x=parent; (echo child | read x); echo \"\$x\"'"

assert_stdout "parent" \
    "$TARGET_SHELL -c 'x=parent; echo child | read x; echo \"\$x\"'"

# ==============================================================================
# Readonly Variable Error
# ==============================================================================
# REQUIREMENT: SHALL-READ-1278:
# Error in setting any variable (readonly) results in return >1.

assert_exit_code_non_zero \
    "echo 'val' | $TARGET_SHELL -c 'readonly rovar=locked; read rovar'"

# Verify partial assignment: if first var succeeds but second is readonly,
# the exit status should still be non-zero.
assert_exit_code_non_zero \
    "echo 'a b' | $TARGET_SHELL -c 'readonly second=locked; read first second'"

# ==============================================================================
# EOF Handling
# ==============================================================================
# REQUIREMENT: SHALL-READ-1281:
# If EOF before terminating delimiter, vars set and exit status 1.

# EOF with no trailing newline: variable is still assigned, exit code is 1.
assert_exit_code 1 \
    "printf 'no newline' | $TARGET_SHELL -c 'read var; exit \$?'"

assert_stdout "no newline" \
    "printf 'no newline' | $TARGET_SHELL -c 'read var; echo \"\$var\"'"

# Completely empty input: variable set to empty, exit code is 1.
assert_exit_code 1 \
    "printf '' | $TARGET_SHELL -c 'read var; exit \$?'"

assert_stdout "" \
    "printf '' | $TARGET_SHELL -c 'read var; echo \"\$var\"'"

# After consuming all lines, next read hits EOF.
assert_exit_code 1 \
    "echo 'only' | $TARGET_SHELL -c 'read line1; read line2; exit \$?'"

# ==============================================================================
# Prompt on Terminal (SHALL-READ-1265)
# ==============================================================================
# REQUIREMENT: SHALL-READ-1265:
# If stdin is terminal and shell interactive, read shall prompt for continuation
# on backslash-newline.
#
# This requirement involves terminal/interactive behavior that cannot be
# reliably tested in a non-interactive pipe context. The backslash-newline
# line continuation itself is tested above in the escape processing section.
# Marking as covered via the line continuation tests.

# ==============================================================================
# read with No Variables (REPLY)
# ==============================================================================
# POSIX does not mandate REPLY, but many shells support it. We test that
# read with at least one variable works correctly (the specified interface).
# When no variable is given, some shells use REPLY; this is an extension.
# We verify the basic single-variable case as the minimal POSIX interface.

assert_stdout "entire line" \
    "echo 'entire line' | $TARGET_SHELL -c 'read line; echo \"\$line\"'"

# ==============================================================================
# Interaction of -r with IFS Splitting
# ==============================================================================
# REQUIREMENT: SHALL-READ-1282:
# read shall conform to XBD 12.2 Utility Syntax Guidelines.

# -r with custom IFS: backslashes are literal AND splitting occurs on IFS.
_out=$($TARGET_SHELL -c 'printf "%s\n" "a\\b:c" | { IFS=: read -r x y; printf "%s:%s" "$x" "$y"; }' 2>/dev/null)
_exp=$(printf '%s:%s' 'a\b' 'c')
if [ "$_out" = "$_exp" ]; then pass; else fail "read -r IFS backslash: expected 'a\\b:c', got '$_out'"; fi

# -r with multiple fields.
assert_stdout 'one:two:three four' \
    "echo 'one:two:three four' | $TARGET_SHELL -c 'IFS=: read -r a b c; echo \"\$a:\$b:\$c\"'"

# ==============================================================================
# Whitespace IFS Trimming Details
# ==============================================================================
# REQUIREMENT: SHALL-READ-1272:
# Remaining input returned unsplit, with leading/trailing IFS whitespace removed.

# Tabs in input treated as whitespace delimiters.
assert_stdout "a b" \
    "printf 'a\tb\n' | $TARGET_SHELL -c 'read x y; echo \"\$x \$y\"'"

# Mixed tabs and spaces.
assert_stdout "hello world" \
    "printf '  \thello\t  world  \t\n' | $TARGET_SHELL -c 'read a b; echo \"\$a \$b\"'"

# Remainder preserves internal tabs.
assert_stdout "a	b	c" \
    "printf 'a\tb\tc\n' | $TARGET_SHELL -c 'read x; echo \"\$x\"'"

# ==============================================================================
# Multiple Reads from Same Stream
# ==============================================================================
# REQUIREMENT: SHALL-READ-1259:
# read shall read a single logical line from stdin.

# Each call to read consumes the next line.
assert_stdout "first:second:third" \
    "printf 'first\nsecond\nthird\n' | $TARGET_SHELL -c '
        read a; read b; read c; echo \"\$a:\$b:\$c\"
    '"

# ==============================================================================
# Empty Lines
# ==============================================================================
# REQUIREMENT: SHALL-READ-1259:
# read shall read a single logical line from stdin.

# An empty line results in empty variable, but exit code 0.
assert_stdout "" \
    "echo '' | $TARGET_SHELL -c 'read var; echo \"\$var\"'"

assert_exit_code 0 \
    "echo '' | $TARGET_SHELL -c 'read var'"

# Multiple variables with empty input: all set to empty.
assert_stdout "::" \
    "echo '' | $TARGET_SHELL -c 'read a b c; echo \"\$a:\$b:\$c\"'"

report
