# SHALL-20-22-03-003
# "When the -v or -V option is used, the command utility shall provide
#  information concerning how a command name is interpreted by the shell."

fail=0

# command -v for a known utility should produce output
out=$(command -v ls)
if [ -z "$out" ]; then
  printf 'FAIL: command -v ls produced no output\n' >&2
  fail=1
fi

# command -V for a known utility should produce output
out=$(command -V ls)
if [ -z "$out" ]; then
  printf 'FAIL: command -V ls produced no output\n' >&2
  fail=1
fi

# Neither should execute the command (ls would list files, but we check
# that the output is informational, not a file listing)
out=$(command -v ls)
case "$out" in
  /*ls*) ;; # pathname — good
  ls) ;; # just the name — fine for builtins
  *) printf 'FAIL: command -v ls unexpected output: %s\n' "$out" >&2; fail=1 ;;
esac

exit "$fail"
