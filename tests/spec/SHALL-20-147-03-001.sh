# SHALL-20-147-03-001
# "The wait utility shall wait for one or more child processes whose process IDs
#  are known in the current shell execution environment to terminate."
# Verify wait blocks until a background process terminates.

_file="$TMPDIR/wait_test_$$"
(sleep 1; printf 'done' > "$_file") &
_pid=$!
wait "$_pid"
_rc=$?

if [ ! -f "$_file" ]; then
  printf '%s\n' "FAIL: wait returned before child completed" >&2
  rm -f "$_file"
  exit 1
fi

rm -f "$_file"
exit 0
