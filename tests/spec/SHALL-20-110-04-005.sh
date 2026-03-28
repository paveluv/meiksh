# SHALL-20-110-04-005
# "Read commands from the command_string operand. Set the value of special
#  parameter 0 ... from the value of the command_name operand and the
#  positional parameters ($1, $2, and so on) in sequence from the remaining
#  argument operands. No commands shall be read from the standard input."
# Verifies: -c sets $0 from command_name, $1/$2 from remaining args.

SH="${MEIKSH:-sh}"

out=$("$SH" -c 'printf "%s %s %s\n" "$0" "$1" "$2"' myname arg1 arg2)
if [ "$out" != "myname arg1 arg2" ]; then
  printf '%s\n' "FAIL: -c args: '$out' expected 'myname arg1 arg2'" >&2; exit 1
fi

exit 0
