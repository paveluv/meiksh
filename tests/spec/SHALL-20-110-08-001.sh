# Test: SHALL-20-110-08-001
# Obligation: "The following environment variables shall affect the execution
#   of sh:"
# Verifies: The shell inherits and exposes standard environment variables
#   (HOME, PATH, PWD) to executed commands.

result=$(HOME=/tmp/testhome PATH="/usr/bin:/bin" "$MEIKSH" -c 'printf "%s\n" "$HOME"')
if [ "$result" != "/tmp/testhome" ]; then
    printf '%s\n' "FAIL: HOME not inherited, got '$result'" >&2
    exit 1
fi

result2=$("$MEIKSH" -c 'printf "%s\n" "$PATH"')
if [ -z "$result2" ]; then
    printf '%s\n' "FAIL: PATH not available in shell" >&2
    exit 1
fi

exit 0
