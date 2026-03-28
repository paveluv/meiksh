# SHALL-20-22-04-008
# "Write a string to standard output that indicates how the name given in the
#  command_name operand will be interpreted by the shell... it shall indicate
#  in which of the following categories command_name falls..."
# -V output must identify the category of each command type.

fail=0

# External utility: identified as such with pathname
out=$(command -V ls)
case "$out" in
  */*ls*) ;; # must contain absolute path
  *) printf 'FAIL: command -V ls missing pathname: %s\n' "$out" >&2; fail=1 ;;
esac

# Shell function: identified as function
myfunc() { :; }
out=$(command -V myfunc)
case "$out" in
  *[Ff]unction*) ;;
  *) printf 'FAIL: command -V myfunc not identified as function: %s\n' "$out" >&2; fail=1 ;;
esac
unset -f myfunc

# Special built-in: identified as special built-in
out=$(command -V export)
case "$out" in
  *[Bb]uilt*) ;;
  *) printf 'FAIL: command -V export not identified as builtin: %s\n' "$out" >&2; fail=1 ;;
esac

# Reserved word: identified as reserved word
out=$(command -V if)
case "$out" in
  *[Rr]eserved*) ;;
  *) printf 'FAIL: command -V if not identified as reserved: %s\n' "$out" >&2; fail=1 ;;
esac

exit "$fail"
