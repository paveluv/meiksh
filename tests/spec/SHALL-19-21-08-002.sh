# Test: SHALL-19-21-08-002
# Obligation: "The following environment variable shall affect the execution
#   of exec: PATH"

# Verify PATH is used to locate exec utility
tmpdir="$TMPDIR/exec_path2_$$"
mkdir -p "$tmpdir"
printf '%s\n' '#!/bin/sh' 'printf "%s" "path_found"' > "$tmpdir/myexeccmd"
chmod +x "$tmpdir/myexeccmd"
result=$(PATH="$tmpdir" exec myexeccmd)
rm -rf "$tmpdir"
if [ "$result" != "path_found" ]; then
    printf '%s\n' "FAIL: PATH not used for exec, got '$result'" >&2
    exit 1
fi

exit 0
