# Test: SHALL-19-13-006
# Obligation: "Unless specified otherwise (see trap), traps that are not being
#   ignored shall be set to the default action."
# Verifies: caught traps are reset to default in subshells; ignored stay ignored.

trap 'echo caught' USR1
trap '' USR2

# In a subshell, caught trap (USR1) should be reset to default
out=$( trap -p USR1 )
if [ -n "$out" ]; then
    printf '%s\n' "FAIL: subshell inherited caught trap: $out" >&2
    exit 1
fi

# In a subshell, ignored trap (USR2) should remain ignored
out=$( trap -p USR2 )
case "$out" in
    *"''"*[Uu][Ss][Rr]2*|*"'' USR2"*|*"'' SIGUSR2"*)
        ;; # correct, still ignored
    *)
        printf '%s\n' "FAIL: subshell lost ignored trap: [$out]" >&2
        exit 1
        ;;
esac

trap - USR1 USR2
exit 0
