# Test: SHALL-19-12-001
# Obligation: "If job control is disabled when the shell executes an
#   asynchronous AND-OR list, the commands in the list shall inherit from
#   the shell a signal action of ignored (SIG_IGN) for the SIGINT and SIGQUIT
#   signals."
# Verifies: async commands under set +m ignore SIGINT.

set +m
trap '' INT
sh -c 'trap - INT; kill -INT $$; printf "alive\n"' &
result=$(wait $!)
# The async child should have survived SIGINT because it inherited SIG_IGN.
# Actually we test indirectly: the child resets trap, sends itself SIGINT.
# Under +m, the async process inherits SIG_IGN which overrides trap -.
# More direct test: check that async child's SIGINT disposition is ignore.
sh -c '
set +m
(trap -p INT) &
wait $!
' > "$TMPDIR/sigint_out.txt" 2>&1

# A simpler approach: run async, send SIGINT, check it survives
set +m
sh -c 'sleep 1' &
child=$!
kill -INT "$child" 2>/dev/null
sleep 1
if wait "$child" 2>/dev/null; then
    : # child survived (exited 0) — SIGINT was ignored
else
    status=$?
    if [ "$status" -gt 128 ]; then
        printf '%s\n' "FAIL: async child died from signal (status=$status)" >&2
        exit 1
    fi
fi
exit 0
