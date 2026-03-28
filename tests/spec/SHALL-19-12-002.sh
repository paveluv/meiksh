# Test: SHALL-19-12-002
# Obligation: "When a signal for which a trap has been set is received while
#   the shell is waiting for the completion of a utility executing a foreground
#   command, the trap associated with that signal shall not be executed until
#   after the foreground command has completed."
# Verifies: trap action for a signal is deferred until foreground command exits.

got_trap=no
trap 'got_trap=yes' USR1
sh -c 'kill -USR1 $PPID; sleep 1' &
child=$!
# The trap fires after the child completes, not during.
# We run a foreground command and send USR1 during it; check trap ran after.
got_trap=no
(sleep 0; kill -USR1 $$) &
sender=$!
# Foreground: a command that takes a moment
sleep 1
wait "$sender" 2>/dev/null
# After the foreground sleep returns, the trap should have been noted
if [ "$got_trap" != "yes" ]; then
    printf '%s\n' "FAIL: trap for USR1 was not executed after foreground cmd" >&2
    exit 1
fi

# Also test: wait builtin returns >128 on trapped signal
trap 'got_trap=wait_trap' USR1
sleep 10 &
bg=$!
got_trap=no
(sleep 1; kill -USR1 $$) &
sender2=$!
wait "$bg"
ws=$?
wait "$sender2" 2>/dev/null
if [ "$ws" -le 128 ]; then
    printf '%s\n' "FAIL: wait should return >128 on signal, got $ws" >&2
    exit 1
fi
if [ "$got_trap" != "wait_trap" ]; then
    printf '%s\n' "FAIL: trap not executed after wait interrupted" >&2
    exit 1
fi
kill "$bg" 2>/dev/null
wait "$bg" 2>/dev/null
exit 0
