# SHALL-19-05-03-024
# "PATH ... A string formatted as described in XBD 8. Environment Variables,
#  used to effect command interpretation; see 2.9.1.4 Command Search and
#  Execution."
# Verify PATH is used for command search.

fail=0

# Create a temp script in a temp directory
dir="$TMPDIR/path_test_$$"
mkdir -p "$dir"
printf '#!/bin/sh\nprintf found\n' > "$dir/mytestcmd"
chmod +x "$dir/mytestcmd"

# With dir in PATH, command should be found
result=$(PATH="$dir:$PATH" "${MEIKSH:-sh}" -c 'mytestcmd')
[ "$result" = "found" ] || { printf '%s\n' "FAIL: PATH search failed: '$result'" >&2; fail=1; }

# Without dir in PATH, command should not be found
PATH=/usr/bin:/bin "${MEIKSH:-sh}" -c 'mytestcmd' >/dev/null 2>&1
[ $? -ne 0 ] || { printf '%s\n' "FAIL: mytestcmd found without PATH entry" >&2; fail=1; }

rm -rf "$dir"

exit "$fail"
