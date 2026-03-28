# Test: SHALL-19-13-002
# Obligation: "If the utility is a shell script, traps caught by the shell
#   shall be set to the default values and traps ignored by the shell shall be
#   set to be ignored by the utility"
# Verifies: caught traps reset to default in child; ignored traps stay ignored.

trap 'echo trapped' USR1
trap '' USR2

# Child script should see USR1 as default (not the parent's handler)
out=$(sh -c 'trap -p USR1' 2>/dev/null)
if [ -n "$out" ]; then
    printf '%s\n' "FAIL: child script inherited caught trap USR1: $out" >&2
    exit 1
fi

# Child script should see USR2 as ignored
out=$(sh -c 'trap -p USR2' 2>/dev/null)
case "$out" in
    *"'' USR2"*|*"'' SIGUSR2"*|*"''"*[Uu][Ss][Rr]2*)
        ;; # good, USR2 is ignored in child
    *)
        printf '%s\n' "FAIL: child script did not inherit ignored USR2: [$out]" >&2
        exit 1
        ;;
esac

trap - USR1 USR2
exit 0
