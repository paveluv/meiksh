# Test: SHALL-19-09-01-06-001
# Obligation: "If the execution is being made via the exec special built-in
#   utility, the shell shall not create a separate utility environment for
#   this execution; the new process image shall replace the current shell
#   execution environment."
# Verifies: exec replaces the shell process (no return to shell).

result=$("$SHELL" -c 'exec printf "%s\n" "replaced"; echo SHOULD_NOT_REACH')
if [ "$result" != "replaced" ]; then
    printf '%s\n' "FAIL: exec did not replace shell process" >&2
    exit 1
fi
case "$result" in
    *SHOULD_NOT_REACH*)
        printf '%s\n' "FAIL: shell continued after exec" >&2
        exit 1
        ;;
esac

exit 0
