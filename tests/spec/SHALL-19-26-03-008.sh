# Test: SHALL-19-26-03-008
# Obligation: "Prevent existing regular files from being overwritten by the
#   shell's '>' redirection operator; the \">|\" redirection operator shall
#   override this noclobber option for an individual file."

tmpfile="$TMPDIR/noclobber_test_$$.txt"
printf '%s\n' "original" > "$tmpfile"

set -C
# > should fail on existing file
if (printf '%s\n' "overwrite" > "$tmpfile") 2>/dev/null; then
    content=$(cat "$tmpfile")
    if [ "$content" = "overwrite" ]; then
        set +C
        rm -f "$tmpfile"
        printf '%s\n' "FAIL: set -C did not prevent overwrite with >" >&2
        exit 1
    fi
fi

# >| should succeed
printf '%s\n' "forced" >| "$tmpfile"
content=$(cat "$tmpfile")
set +C
rm -f "$tmpfile"
if [ "$content" != "forced" ]; then
    printf '%s\n' "FAIL: >| did not override noclobber" >&2
    exit 1
fi

exit 0
