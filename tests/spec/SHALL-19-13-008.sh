# Test: SHALL-19-13-008
# Obligation: "Unless specified otherwise (see trap), traps that are not being
#   ignored shall be set to the default action."
# (Duplicate of SHALL-19-13-006)
# Verifies: non-ignored traps reset in subshell; ignored traps preserved.

trap 'echo caught' USR1
trap '' USR2

out=$( trap -p USR1 )
if [ -n "$out" ]; then
    printf '%s\n' "FAIL: subshell inherited caught trap USR1: $out" >&2
    exit 1
fi

out=$( trap -p USR2 )
case "$out" in
    *"''"*[Uu][Ss][Rr]2*|*"'' USR2"*|*"'' SIGUSR2"*) ;;
    *) printf '%s\n' "FAIL: subshell lost ignored trap USR2: [$out]" >&2; exit 1 ;;
esac

trap - USR1 USR2
exit 0
