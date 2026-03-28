# SHALL-20-22-10-002
# "When the -V option is specified, standard output shall be formatted as:
#  \"%s\\n\", <unspecified>"
# Each command -V result is a single line terminated by newline.

fail=0

out=$(command -V ls)
lines=$(printf '%s\n' "$out" | wc -l)
lines=$(echo "$lines" | tr -d ' ')
if [ "$lines" -ne 1 ]; then
  printf 'FAIL: command -V ls should be 1 line, got %s\n' "$lines" >&2
  fail=1
fi
if [ -z "$out" ]; then
  printf 'FAIL: command -V ls produced empty output\n' >&2
  fail=1
fi

exit "$fail"
