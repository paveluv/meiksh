# SHALL-20-22-04-006
# "Write a string to standard output that indicates the pathname or command
#  that will be used by the shell... Executable utilities... shall be written
#  as absolute pathnames. Shell functions, special built-in utilities, regular
#  built-in utilities not associated with a PATH search, and shell reserved
#  words shall be written as just their names. An alias shall be written as a
#  command line that represents its alias definition."

fail=0

# External utility: absolute pathname
out=$(command -v ls)
case "$out" in
  /*) ;;
  *) printf 'FAIL: command -v ls not absolute path: %s\n' "$out" >&2; fail=1 ;;
esac

# Shell function: just the name
myfunc() { :; }
out=$(command -v myfunc)
if [ "$out" != "myfunc" ]; then
  printf 'FAIL: command -v myfunc expected "myfunc", got "%s"\n' "$out" >&2
  fail=1
fi
unset -f myfunc

# Special built-in: just the name
out=$(command -v export)
if [ "$out" != "export" ]; then
  printf 'FAIL: command -v export expected "export", got "%s"\n' "$out" >&2
  fail=1
fi

# Reserved word: just the name
out=$(command -v if)
if [ "$out" != "if" ]; then
  printf 'FAIL: command -v if expected "if", got "%s"\n' "$out" >&2
  fail=1
fi

# Not found: no output, nonzero exit
out=$(command -v __no_such_cmd_98765__ 2>/dev/null)
rc=$?
if [ "$rc" -eq 0 ]; then
  printf 'FAIL: command -v for nonexistent should exit nonzero\n' >&2
  fail=1
fi
if [ -n "$out" ]; then
  printf 'FAIL: command -v for nonexistent should produce no output\n' >&2
  fail=1
fi

exit "$fail"
