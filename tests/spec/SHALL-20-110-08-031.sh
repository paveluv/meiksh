# reviewed: GPT-5.4
# Test: SHALL-20-110-08-031
# Obligation: "Establish a string formatted as described in XBD 8. Environment
#   Variables, used to effect command interpretation; see 2.9.1.4 Command
#   Search and Execution."
# Verifies: PATH is used for command search.

SH="${MEIKSH:-${SHELL:-sh}}"

mkdir -p "$TMPDIR/pathtest"
printf 'printf "%%s\\n" "found-it"\n' > "$TMPDIR/pathtest/mytestcmd"
chmod +x "$TMPDIR/pathtest/mytestcmd"

result=$(PATH="$TMPDIR/pathtest" "$SH" -c 'mytestcmd')
if [ "$result" != "found-it" ]; then
    printf '%s\n' "FAIL: PATH not used for command search, got '$result'" >&2
    exit 1
fi

exit 0
