# Test: SHALL-19-13-005
# Obligation: "A subshell environment shall be created as a duplicate of the
#   shell environment, except that:"
# Verifies: subshell inherits variables, functions, and cwd from parent.

MY_VAR=hello
my_func() { printf '%s\n' "func_works"; }
export MY_VAR

got_var=$(MY_VAR=hello; printf '%s\n' "$MY_VAR")
if [ "$got_var" != "hello" ]; then
    printf '%s\n' "FAIL: subshell did not inherit variable" >&2
    exit 1
fi

orig_dir=$(pwd)
sub_dir=$(cd /tmp && pwd)
if [ "$(pwd)" != "$orig_dir" ]; then
    printf '%s\n' "FAIL: subshell cd changed parent cwd" >&2
    exit 1
fi
exit 0
