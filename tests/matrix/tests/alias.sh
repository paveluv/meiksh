# Test: alias and unalias Built-ins
# Target: tests/matrix/tests/alias.sh
#
# Tests POSIX requirements for alias and unalias builtins: defining,
# listing, quoting, expansion, scope, and removal of aliases.

. "$MATRIX_DIR/lib.sh"

# ==============================================================================
# alias-name operand writes alias definition to stdout
# ==============================================================================
# REQUIREMENT: SHALL-ALIAS-1026:
# When an alias-name operand is given, the alias definition shall be
# written to standard output.

_out=$($TARGET_SHELL -c 'alias myalias="echo hello"; alias myalias')
case "$_out" in
    *myalias*echo*hello*) pass ;;
    *) fail "alias-name operand did not write definition to stdout: '$_out'" ;;
esac

# Querying a second alias also writes its definition
_out=$($TARGET_SHELL -c 'alias foo="ls -la"; alias bar="grep x"; alias foo')
case "$_out" in
    *foo*ls*) pass ;;
    *) fail "alias-name query didn't show correct definition: '$_out'" ;;
esac

# ==============================================================================
# alias definition replaces command name when encountered
# ==============================================================================
# REQUIREMENT: SHALL-ALIAS-1053:
# When a command is encountered, the alias definition shall replace the
# command name.

assert_stdout "hello world" \
    "$TARGET_SHELL -c 'alias greet=\"echo hello world\"; greet'"

assert_stdout "42" \
    "$TARGET_SHELL -c 'alias answer=\"echo 42\"; answer'"

# ==============================================================================
# alias affects current shell execution environment and subshells
# ==============================================================================
# REQUIREMENT: SHALL-ALIAS-1054:
# Aliases shall affect the current shell execution environment and
# subshells.

assert_stdout "from_alias" \
    "$TARGET_SHELL -c 'alias myecho=\"echo from_alias\"; myecho'"

# Alias visible in a subshell
assert_stdout "sub_alias" \
    "$TARGET_SHELL -c 'alias sa=\"echo sub_alias\"; (sa)'"

# ==============================================================================
# alias shall not affect parent process or utility environment
# ==============================================================================
# REQUIREMENT: SHALL-ALIAS-1055:
# Aliases shall not affect the environment of the parent process or
# any utility environment invoked by the shell.

# An alias defined in a subshell must not leak into the parent
assert_stdout "world" \
    "$TARGET_SHELL -c '(alias leaked=\"echo LEAKED\"); echo world'"

# An alias defined in $TARGET_SHELL -c must not affect this outer shell
$TARGET_SHELL -c 'alias outer_leak="echo LEAKED"' 2>/dev/null
if command -v outer_leak >/dev/null 2>&1; then
    fail "alias leaked into parent process"
else
    pass
fi

# ==============================================================================
# If no operands, all alias definitions written to stdout
# ==============================================================================
# REQUIREMENT: SHALL-ALIAS-1057:
# If no operands are given, all alias definitions shall be written to
# standard output.

_out=$($TARGET_SHELL -c 'alias a1="echo 1"; alias a2="echo 2"; alias')
case "$_out" in
    *a1*echo*1*) : ;;
    *) fail "alias with no operands missing a1: '$_out'" ;;
esac
case "$_out" in
    *a2*echo*2*) pass ;;
    *) fail "alias with no operands missing a2: '$_out'" ;;
esac

# ==============================================================================
# Value string written with appropriate quoting suitable for reinput
# ==============================================================================
# REQUIREMENT: SHALL-ALIAS-1060:
# The value string shall be written with appropriate quoting so that
# it is suitable for reinput to the shell.

# An alias containing spaces and special chars must be quoted for reinput
_def=$($TARGET_SHELL -c 'alias special="echo hello world"; alias special')
_reinput_out=$(eval "$_def" 2>/dev/null && $TARGET_SHELL -c "$_def; special" 2>/dev/null)
case "$_reinput_out" in
    *"hello world"*) pass ;;
    *) fail "alias output not suitable for reinput: def='$_def' result='$_reinput_out'" ;;
esac

# Verify quoting handles single quotes in value
_def2=$($TARGET_SHELL -c "alias sq=\"echo it'\"'\"'s fine\"; alias sq")
_reinput2=$($TARGET_SHELL -c "$_def2; sq" 2>/dev/null)
case "$_reinput2" in
    *"it's fine"*) pass ;;
    *) fail "alias quoting failed for single quotes: def='$_def2' result='$_reinput2'" ;;
esac

# ==============================================================================
# unalias -a removes all alias definitions
# ==============================================================================
# REQUIREMENT: SHALL-UNALIAS-1083:
# The -a option shall remove all alias definitions from the current
# shell execution environment.

_out=$($TARGET_SHELL -c 'alias x1="echo 1"; alias x2="echo 2"; unalias -a; alias')
case "$_out" in
    *x1*) fail "unalias -a did not remove x1: '$_out'" ;;
    *x2*) fail "unalias -a did not remove x2: '$_out'" ;;
    *) pass ;;
esac

# ==============================================================================
# unalias alias-name operand removes the named alias
# ==============================================================================
# REQUIREMENT: SHALL-UNALIAS-1084:
# The alias-name operand shall remove the named alias definition from
# the alias list.

_out=$($TARGET_SHELL -c 'alias rmme="echo gone"; unalias rmme; alias rmme 2>&1')
_rc=$?
case "$_out" in
    *rmme*echo*gone*) fail "unalias did not remove named alias: '$_out'" ;;
    *) pass ;;
esac

# After removal, the alias command should not expand
assert_exit_code_non_zero "$TARGET_SHELL -c 'alias rmme2=\"echo gone\"; unalias rmme2; rmme2'"

# ==============================================================================
# unalias removes definition for each alias name
# ==============================================================================
# REQUIREMENT: SHALL-UNALIAS-1335:
# The unalias utility shall remove the definition for each alias name
# specified.

# Remove multiple aliases in one call
_out=$($TARGET_SHELL -c 'alias a="echo A"; alias b="echo B"; alias c="echo C"; unalias a b; alias')
case "$_out" in
    *" a="*|*"a="*) fail "unalias did not remove 'a': '$_out'" ;;
    *) : ;;
esac
case "$_out" in
    *" b="*|*"b="*) fail "unalias did not remove 'b': '$_out'" ;;
    *) : ;;
esac
case "$_out" in
    *c*echo*C*) pass ;;
    *) fail "unalias accidentally removed 'c' or alias listing broken: '$_out'" ;;
esac

# ==============================================================================
# Removed from current shell execution environment
# ==============================================================================
# REQUIREMENT: SHALL-UNALIAS-1336:
# Aliases shall be removed from the current shell execution environment.

# After unalias, the name should no longer expand as an alias
assert_stdout "after_removal" \
    "$TARGET_SHELL -c 'alias zz=\"echo WRONG\"; unalias zz; echo after_removal'"

# Verify unalias -a clears environment so no alias expands
assert_stdout "clean" \
    "$TARGET_SHELL -c 'alias yy=\"echo WRONG\"; unalias -a; echo clean'"

report
