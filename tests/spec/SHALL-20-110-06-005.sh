# reviewed: GPT-5.4
# Test: SHALL-20-110-06-005
# Obligation: "When the shell is using standard input and it invokes a command
#   that also uses standard input, the shell shall ensure that the standard
#   input file pointer points directly after the command it has read when the
#   command begins execution."
# Verifies: when sh reads commands from stdin, it does not read ahead past the
#   current parsed command line before starting a stdin-reading command.

SH="${MEIKSH:-${SHELL:-sh}}"

result=$(printf 'read a; printf "%%s\\n" "$a"\nnextline\n' | "$SH")
if [ "$result" != 'nextline' ]; then
    printf '%s\n' "FAIL: stdin read-ahead handling wrong, got '$result'" >&2
    exit 1
fi

exit 0
