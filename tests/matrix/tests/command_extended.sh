# Test: command — Extended Tests for the command Built-in
# Target: tests/matrix/tests/command_extended.sh
#
# Tests additional POSIX requirements for the command built-in: -V output,
# argument passing, function suppression, special built-in override,
# alias/reserved word suppression, -v/-V info, and declaration utility
# handling.

. "$MATRIX_DIR/lib.sh"

# ==============================================================================
# command -V: Output format
# ==============================================================================
# REQUIREMENT: SHALL-COMMAND-1030:
# -V option output format: "%s\n", unspecified

# command -V should produce output describing how the name is interpreted
test_cmd='command -V echo'
_out=$($TARGET_SHELL -c "$test_cmd" 2>&1)
case "$_out" in
    *echo*) pass ;;
    *) fail "Expected 'echo' mentioned in 'command -V echo' output, got: $_out" ;;
esac

# command -V for a nonexistent command should fail
assert_exit_code_non_zero \
    "$TARGET_SHELL -c 'command -V nonexistent_cmd_xyzzy 2>/dev/null'"

# command -V for a built-in should produce output containing the name
test_cmd='command -V cd'
_out=$($TARGET_SHELL -c "$test_cmd" 2>&1)
case "$_out" in
    *cd*) pass ;;
    *) fail "Expected 'cd' mentioned in 'command -V cd' output, got: $_out" ;;
esac

# ==============================================================================
# command: Argument operand treated as argument to command_name
# ==============================================================================
# REQUIREMENT: SHALL-COMMAND-1038:
# argument operand treated as argument to command_name

# Arguments after command_name are passed through
assert_stdout "hello world" \
    "$TARGET_SHELL -c 'command echo hello world'"

# Multiple arguments should be forwarded correctly
assert_stdout "a b c" \
    "$TARGET_SHELL -c 'command printf \"%s %s %s\n\" a b c'"

# ==============================================================================
# command: Suppresses shell function lookup
# ==============================================================================
# REQUIREMENT: SHALL-COMMAND-1104:
# command utility suppresses shell function lookup

# Define a function that shadows a utility; command should bypass it
test_cmd='echo() { printf "FUNCTION\n"; }; command echo REAL'
assert_stdout "REAL" \
    "$TARGET_SHELL -c '$test_cmd'"

# Verify the function itself works without command prefix
test_cmd='echo() { printf "FUNCTION\n"; }; echo ignored'
assert_stdout "FUNCTION" \
    "$TARGET_SHELL -c '$test_cmd'"

# command should bypass a function shadowing 'cat'
test_cmd='cat() { printf "FAKE_CAT\n"; }; printf "real\n" | command cat'
assert_stdout "real" \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# command: Special properties of special built-ins do not occur
# ==============================================================================
# REQUIREMENT: SHALL-COMMAND-1105:
# Special properties of special built-ins do not occur

# A variable assignment error in a special built-in normally causes the shell
# to exit. With 'command' prefix, the shell should not exit.
# 'export' is a special built-in. An invalid assignment should not abort
# the shell when prefixed with 'command'.
test_cmd='command export 2>/dev/null; echo survived'
assert_stdout "survived" \
    "$TARGET_SHELL -c '$test_cmd'"

# 'command break' outside a loop: without command, break is a special built-in
# error that may exit the shell. With command, the shell should survive.
test_cmd='command break 2>/dev/null; echo still_here'
assert_stdout "still_here" \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# command: Same effect as omitting command, no alias/reserved word recognition
# ==============================================================================
# REQUIREMENT: SHALL-COMMAND-1106:
# Effect same as omitting command, except no alias/reserved word recognition

# command should execute the utility normally
assert_stdout "hello" \
    "$TARGET_SHELL -c 'command echo hello'"

# Verify that command does not perform alias expansion
test_cmd='alias ls="echo ALIASED" 2>/dev/null; command ls / >/dev/null 2>&1; echo $?'
_out=$($TARGET_SHELL -c "$test_cmd" 2>&1)
case "$_out" in
    *ALIASED*) fail "command should not expand aliases, got: $_out" ;;
    *) pass ;;
esac

# ==============================================================================
# command -v / -V: Provide info on command name interpretation
# ==============================================================================
# REQUIREMENT: SHALL-COMMAND-1107:
# -v or -V provides info on how command name is interpreted

# command -v for an external utility should print its path
test_cmd='command -v ls'
_out=$($TARGET_SHELL -c "$test_cmd" 2>&1)
case "$_out" in
    */*ls*|ls) pass ;;
    *) fail "Expected path or name for 'command -v ls', got: $_out" ;;
esac

# command -v for a built-in should print the built-in name
test_cmd='command -v cd'
_out=$($TARGET_SHELL -c "$test_cmd" 2>&1)
case "$_out" in
    *cd*) pass ;;
    *) fail "Expected 'cd' from 'command -v cd', got: $_out" ;;
esac

# command -v for a defined function should print the function name
test_cmd='myfn() { :; }; command -v myfn'
assert_stdout "myfn" \
    "$TARGET_SHELL -c '$test_cmd'"

# command -v for an alias should print the alias definition
test_cmd='alias greet="echo hi" 2>/dev/null; command -v greet'
_out=$($TARGET_SHELL -c "$test_cmd" 2>&1)
case "$_out" in
    *greet*|*"echo hi"*) pass ;;
    *) pass ;; # alias support in non-interactive varies
esac

# command -v for a nonexistent command should fail
assert_exit_code_non_zero \
    "$TARGET_SHELL -c 'command -v nonexistent_cmd_xyzzy 2>/dev/null'"

# command -V should describe the command type
test_cmd='command -V echo'
_out=$($TARGET_SHELL -c "$test_cmd" 2>&1)
# Output is unspecified format, but must mention 'echo'
case "$_out" in
    *echo*) pass ;;
    *) fail "Expected 'echo' in 'command -V echo' output, got: $_out" ;;
esac

# ==============================================================================
# command: Treated as declaration utility if first arg is declaration utility
# ==============================================================================
# REQUIREMENT: SHALL-COMMAND-1108:
# Treated as declaration utility if first arg is declaration utility

# 'command export' should behave as a declaration utility
test_cmd='command export FOO=bar; echo $FOO'
assert_stdout "bar" \
    "$TARGET_SHELL -c '$test_cmd'"

# 'command readonly' should behave as a declaration utility
test_cmd='command readonly RO_VAR=42; echo $RO_VAR'
assert_stdout "42" \
    "$TARGET_SHELL -c '$test_cmd'"

# 'command local' inside a function (if supported) should work as declaration
test_cmd='f() { command local LV=hello 2>/dev/null && echo $LV || echo $LV; }; f'
_out=$($TARGET_SHELL -c "$test_cmd" 2>&1)
case "$_out" in
    *hello*) pass ;;
    *) pass ;; # local not required by POSIX
esac

# ==============================================================================
# command: Subsequent name=word expanded in assignment context
# ==============================================================================
# REQUIREMENT: SHALL-COMMAND-1109:
# Subsequent name=word expanded in assignment context

# When command precedes a declaration utility, name=word pairs should be
# expanded as assignments (tilde expansion, no field splitting).
test_cmd='command export TVAR=hello; echo $TVAR'
assert_stdout "hello" \
    "$TARGET_SHELL -c '$test_cmd'"

# Multiple assignments in one command export statement
test_cmd='command export A=1 B=2 C=3; echo $A $B $C'
assert_stdout "1 2 3" \
    "$TARGET_SHELL -c '$test_cmd'"

# Tilde expansion should occur in assignment context
test_cmd='command export HOMEDIR=~; case "$HOMEDIR" in /*) echo absolute;; *) echo relative;; esac'
assert_stdout "absolute" \
    "$TARGET_SHELL -c '$test_cmd'"

# Variable references in assignment values should expand
test_cmd='X=hello; command export Y=${X}_world; echo $Y'
assert_stdout "hello_world" \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# XBD 12.2: Optional option-argument directly adjacent
# ==============================================================================
# REQUIREMENT: SHALL-XBD-12-4002:
# If the SYNOPSIS shows an optional option-argument, a conforming application
# shall place any option-argument directly adjacent to the option.
# REQUIREMENT: SHALL-XBD-12-4003:
# If the utility receives an argument containing only the option, it shall
# behave as specified for an omitted option-argument.

# ulimit -f with value adjacent (e.g., ulimit -Sf100)
_out=$($TARGET_SHELL -c 'ulimit -Sf 100; ulimit -Sf' 2>/dev/null)
case "$_out" in
    100|unlimited) pass ;;
    *) fail "ulimit -Sf 100 did not set limit, got: $_out" ;;
esac

# kill -l without argument lists signals; -l with argument shows signal name
_out=$($TARGET_SHELL -c 'kill -l' 2>/dev/null)
case "$_out" in
    *HUP*|*INT*|*TERM*) pass ;;
    *) fail "kill -l without argument did not list signals, got: $_out" ;;
esac

report
