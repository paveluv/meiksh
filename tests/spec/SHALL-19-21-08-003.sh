# Test: SHALL-19-21-08-003
# Obligation: "Determine the search path when looking for the utility given as
#   the utility operand; see XBD 8.3 Other Environment Variables."

# exec with slash in path does not use PATH
tmpfile="$TMPDIR/exec_slash_$$.sh"
printf '%s\n' '#!/bin/sh' 'printf "%s" "direct_path"' > "$tmpfile"
chmod +x "$tmpfile"
result=$(exec "$tmpfile")
rm -f "$tmpfile"
if [ "$result" != "direct_path" ]; then
    printf '%s\n' "FAIL: exec with slash path did not work" >&2
    exit 1
fi

exit 0
