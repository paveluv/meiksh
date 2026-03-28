# Test: SHALL-12-02-001
# Obligation: "The -W (capital-W) option shall be reserved for vendor options.
#   Multi-digit options should not be allowed."
# Verifies: Standard shell builtins do not use -W for standard functionality.
#   When -W is passed to a builtin that doesn't define it, it is rejected.

# 'cd' should not accept -W as a standard option
out=$(${MEIKSH:-meiksh} -c 'cd -W / 2>&1' 2>&1)
rc=$?
if [ "$rc" -eq 0 ] && [ -z "$out" ]; then
    printf '%s\n' "FAIL: cd -W should either be rejected or be vendor extension, got silent success" >&2
    exit 1
fi

# 'read' should not accept -W as a standard option
out=$(${MEIKSH:-meiksh} -c 'printf "hello\n" | read -W var 2>&1' 2>&1)
rc=$?
if [ "$rc" -eq 0 ] && [ -z "$out" ]; then
    printf '%s\n' "FAIL: read -W should either be rejected or be vendor extension, got silent success" >&2
    exit 1
fi

exit 0
