# SHALL-08-03-006
# "This variable shall represent the sequence of path prefixes that certain
#  functions and utilities apply in searching for an executable file."
# Verify PATH search: command found via PATH, slash bypasses PATH.

_dir="$TMPDIR/shall080306.$$"
mkdir -p "$_dir"
printf '#!/bin/sh\nprintf found\n' > "$_dir/cmd080306"
chmod +x "$_dir/cmd080306"

# PATH search finds command
_out=$(PATH="$_dir" cmd080306 2>&1)
if [ "$_out" != "found" ]; then
  rm -rf "$_dir"
  printf '%s\n' "FAIL: PATH search did not find command" >&2
  exit 1
fi

# Command with slash bypasses PATH
_out=$("$_dir/cmd080306" 2>&1)
if [ "$_out" != "found" ]; then
  rm -rf "$_dir"
  printf '%s\n' "FAIL: absolute path did not bypass PATH" >&2
  exit 1
fi

# Command not in PATH should fail
_out=$(PATH=/nonexistent cmd080306 2>/dev/null)
_rc=$?
if [ "$_rc" -eq 0 ]; then
  rm -rf "$_dir"
  printf '%s\n' "FAIL: command found with wrong PATH" >&2
  exit 1
fi

rm -rf "$_dir"
exit 0
