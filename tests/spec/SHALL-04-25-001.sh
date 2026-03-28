# Test: SHALL-04-25-001
# Obligation: "A utility program shall be either an executable file, such as
#   might be produced by a compiler or linker system from computer source code,
#   or a file of shell source code, directly interpreted by the shell."
# Verifies: The shell can execute both binary executables and shell scripts
#   found via PATH.

# Test 1: Binary executable (use a standard POSIX utility)
result=$(true && printf '%s\n' "ok")
if [ "$result" != "ok" ]; then
    echo "FAIL: could not execute binary utility 'true'" >&2
    exit 1
fi

# Test 2: Shell script utility
mkdir -p "$TMPDIR/bin"
cat > "$TMPDIR/bin/testutil" <<'SCRIPT'
printf '%s\n' "from-script"
SCRIPT
chmod +x "$TMPDIR/bin/testutil"
OLD_PATH=$PATH
PATH="$TMPDIR/bin:$PATH"
result=$(testutil)
PATH=$OLD_PATH
if [ "$result" != "from-script" ]; then
    echo "FAIL: shell script utility not executed, got '$result'" >&2
    exit 1
fi

exit 0
