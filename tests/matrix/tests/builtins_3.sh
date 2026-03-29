# Test: Special Built-ins (export, readonly, set, unset)
# Target: tests/matrix/tests/builtins_3.sh
#
# POSIX Shell mandates special built-ins for managing variables and shell
# state. This suite tests the behavior of `export`, `readonly`, `set`,
# and `unset` utilities.

. "$MATRIX_DIR/lib.sh"

# ==============================================================================
# The 'export' Utility
# ==============================================================================
# REQUIREMENT: SHALL-DESCRIPTION-558:
# The shell shall give the export attribute to the variables corresponding to
# the specified name s, which shall cause them to be in the environment of
# subsequently executed commands.
# REQUIREMENT: SHALL-DESCRIPTION-559:
# If the name of a variable is followed by = word , then the value of that
# variable shall be set to word .
# REQUIREMENT: SHALL-DESCRIPTION-562:
# The export special built-in shall support XBD 12.2 Utility Syntax Guidelines
# .

# Exporting a variable makes it available to child processes. We test both
# exporting an existing variable and exporting while assigning.
test_cmd='
foo="bar"
export foo baz="qux"
env | grep -E "^(foo|baz)=" | sort
'
assert_stdout "baz=qux
foo=bar" \
    "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-DESCRIPTION-580:
# When -p is specified, export shall write
# to the standard output the names and values of all exported variables...
# REQUIREMENT: SHALL-DESCRIPTION-645:
# The shell shall format the output,
# including the proper use of quoting, so that it is suitable for reinput...

# `export -p` must generate valid shell commands. We test this by eval-ing it.
test_cmd='
export EXPORTED_VAR="val with spaces"
output=$(export -p | grep "EXPORTED_VAR=")
# It should be eval-able and set the variable
unset EXPORTED_VAR
eval "$output"
echo "$EXPORTED_VAR"
'
assert_stdout "val with spaces" \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# The 'readonly' Utility
# ==============================================================================
# REQUIREMENT: SHALL-Issue-6-566:
# IEEE Std 1003.1-2001/Cor 1-2002, item XCU/TC1/D6/6 is applied, adding the
# following text to the end of the first paragraph of the DESCRIPTION: "If the
# name of a variable is followed by = word , then the value of that variable
# shall be set to word .".
# REQUIREMENT: SHALL-Issue-6-575:
# IEEE Std 1003.1-2001/Cor 1-2002, item XCU/TC1/D6/7 is applied, adding the
# following text to the end of the first paragraph of the DESCRIPTION: "If the
# name of a variable is followed by = word , then the value of that variable
# shall be set to word .".
# REQUIREMENT: SHALL-Issue-6-628:
# IEEE Std 1003.1-2001/Cor 1-2002, item XCU/TC1/D6/9 is applied, changing text
# in the DESCRIPTION from: "Write the accumulated user and system times for the
# shell and for all of its child processes ..." to: "The times utility shall
# write the accumulated user and system times for the shell and for all of its
# child processes ...".
# REQUIREMENT: SHALL-DESCRIPTION-567:
# The variables whose name s are specified shall be given the readonly
# attribute.
# REQUIREMENT: SHALL-DESCRIPTION-568:
# As described in XBD 8.1 Environment Variable Definition , conforming
# applications shall not request to mark a variable as readonly if it is
# documented as being manipulated by a shell built-in utility, as it may render
# those utilities unable to complete successfully.
# REQUIREMENT: SHALL-DESCRIPTION-559:
# If the name of a variable is followed by = word , then the value of that
# variable shall be set to word .
# REQUIREMENT: SHALL-DESCRIPTION-572:
# The readonly special built-in shall support XBD 12.2 Utility Syntax
# Guidelines .

# Attempting to assign to a readonly variable fails.
test_cmd='readonly RO_VAR="protected"; RO_VAR="mutated"'
assert_exit_code_non_zero \
    "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-DESCRIPTION-645:
# The shell shall format the output [of -p],
# including the proper use of quoting, so that it is suitable for reinput...

test_cmd='
readonly RO_VAR="protected"
output=$(readonly -p | grep "RO_VAR=")
# It should be eval-able and set the variable in a new shell
echo "$output; echo \"\$RO_VAR\"" > tmp_ro.sh
'"$TARGET_SHELL"' tmp_ro.sh
'
assert_stdout "protected" \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# The 'set' Utility
# ==============================================================================
# REQUIREMENT: SHALL-DESCRIPTION-580:
# If no option s or argument s are specified, set shall write the names and
# values of all shell variables in the collation sequence of the current locale.

test_cmd='
MY_TEST_VAR="hello_set"
set | grep -q "^MY_TEST_VAR=hello_set$" && echo "found"
'
assert_stdout "found" \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# The 'unset' Utility
# ==============================================================================
# REQUIREMENT: SHALL-DESCRIPTION-617:
# The positional parameters shall be shifted.
# REQUIREMENT: SHALL-DESCRIPTION-618:
# Positional parameter 1 shall be assigned the value of parameter (1+ n ),
# parameter 2 shall be assigned the value of parameter (2+ n ), and so on.
# REQUIREMENT: SHALL-DESCRIPTION-651:
# The unset utility shall unset each variable or function definition specified
# by name that does not have the readonly attribute and remove any attributes
# other than readonly that have been given to name (see 2.15 Special Built-In
# Utilities export and readonly ).
# REQUIREMENT: SHALL-DESCRIPTION-652:
# If -v is specified, name refers to a variable name and the shell shall unset
# it and remove it from the environment.
# REQUIREMENT: SHALL-DESCRIPTION-653:
# If -f is specified, name refers to a function and the shell shall unset the
# function definition.
# REQUIREMENT: SHALL-DESCRIPTION-654:
# If neither -f nor -v is specified, name refers to a variable; if a variable
# by that name does not exist, it is unspecified whether a function by that
# name, if any, shall be unset.
# REQUIREMENT: SHALL-DESCRIPTION-655:
# Unsetting a variable or function that was not previously set shall not be
# considered an error and does not cause the shell to abort.
# REQUIREMENT: SHALL-DESCRIPTION-656:
# The unset special built-in shall support XBD 12.2 Utility Syntax Guidelines .

test_cmd='
my_var="value"
unset my_var
echo "${my_var:-is_unset}"
'
assert_stdout "is_unset" \
    "$TARGET_SHELL -c '$test_cmd'"

# Unsetting a function using `-f`.
# REQUIREMENT: SHALL-DESCRIPTION-620:
# The value n shall be an unsigned decimal integer less than or equal to the
# value of the special parameter '#' .

test_cmd='
my_func() { echo "running"; }
unset -f my_func
my_func 2>/dev/null || echo "not found"
'
assert_stdout "not found" \
    "$TARGET_SHELL -c '$test_cmd'"


report
