# Test: SHALL-04-26-001
# Obligation: "the value (representing a word or field) shall be assigned as
#   the value of the variable denoted by varname" in assignment context only:
#   "Assignment context occurs in the cmd_prefix portion of a shell simple
#   command, as well as in arguments of a recognized declaration utility."
# Verifies: Variable assignment works in simple command prefix and that
#   name=value is NOT treated as an assignment outside assignment context.

# Assignment in simple command prefix (no command name: permanent)
FOO=hello
if [ "$FOO" != "hello" ]; then
    echo "FAIL: prefix assignment FOO=hello did not set FOO" >&2
    exit 1
fi

# Assignment in simple command prefix (with command): only for that command
BAR=world true
if [ -n "$BAR" ]; then
    echo "FAIL: prefix assignment with command leaked BAR into shell env" >&2
    exit 1
fi

# name=value as argument is NOT an assignment
unset ZAP
eval 'printf "%s\n" ZAP=oops' >/dev/null 2>&1
if [ -n "$ZAP" ]; then
    echo "FAIL: ZAP=oops as argument to printf should not assign" >&2
    exit 1
fi

# export (declaration utility) recognizes assignment in arguments
export DECL_TEST=exported_val
if [ "$DECL_TEST" != "exported_val" ]; then
    echo "FAIL: export DECL_TEST=exported_val did not assign" >&2
    exit 1
fi

exit 0
