# SHALL-20-22-10-001
# "When the -v option is specified, standard output shall be formatted as:
#  \"%s\\n\", <pathname or command>"
# Each command -v result is a single line terminated by newline.

fail=0

out=$(command -v ls)
# Must end with a newline (printf will have stripped it, but there should be
# exactly one line)
lines=$(printf '%s\n' "$out" | wc -l)
lines=$(echo "$lines" | tr -d ' ')
if [ "$lines" -ne 1 ]; then
  printf 'FAIL: command -v ls should be 1 line, got %s\n' "$lines" >&2
  fail=1
fi

# output should be non-empty
if [ -z "$out" ]; then
  printf 'FAIL: command -v ls produced empty output\n' >&2
  fail=1
fi

exit "$fail"
