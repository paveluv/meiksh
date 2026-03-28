# Test: SHALL-19-21-08-001
# Obligation: "The following environment variable shall affect the execution
#   of exec:"

# PATH affects exec's search for utility
tmpdir="$TMPDIR/exec_path_$$"
mkdir -p "$tmpdir"
printf '%s\n' '#!/bin/sh' 'printf "%s" "exec_path_ok"' > "$tmpdir/exec_test_cmd"
chmod +x "$tmpdir/exec_test_cmd"
result=$(PATH="$tmpdir:$PATH" exec exec_test_cmd)
rm -rf "$tmpdir"
if [ "$result" != "exec_path_ok" ]; then
    printf '%s\n' "FAIL: PATH did not affect exec, got '$result'" >&2
    exit 1
fi

exit 0
