# SHALL-09-03-05-002
# "When the bracket expression appears within a shell pattern ... the special
#  characters '?', '*', and '[' ... shall lose their special meaning within
#  the bracket expression"
# Verify ?, *, [ are literal inside bracket expressions in shell patterns.

# * inside bracket expression matches literal *
_f="$TMPDIR/shall090305002.$$"
mkdir -p "$_f"
: > "$_f/*"
: > "$_f/a"
_count=$(cd "$_f" && set -- [*]; printf '%s\n' "$#")
rm -rf "$_f"
if [ "$_count" != "1" ]; then
  printf '%s\n' "FAIL: [*] did not match literal * only" >&2
  exit 1
fi

# ? inside bracket expression matches literal ?
_f="$TMPDIR/shall090305002b.$$"
mkdir -p "$_f"
: > "$_f/?"
: > "$_f/a"
_count=$(cd "$_f" && set -- [?]; printf '%s\n' "$#")
rm -rf "$_f"
if [ "$_count" != "1" ]; then
  printf '%s\n' "FAIL: [?] did not match literal ? only" >&2
  exit 1
fi

exit 0
