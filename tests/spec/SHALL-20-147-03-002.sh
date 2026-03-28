# SHALL-20-147-03-002
# "If the wait utility is invoked with no operands, it shall wait until all
#  process IDs known to the invoking shell have terminated and exit with a
#  zero exit status."
# Verify wait with no args waits for all children and exits 0.

_f1="$TMPDIR/wait_all1_$$"
_f2="$TMPDIR/wait_all2_$$"
rm -f "$_f1" "$_f2"

(sleep 1; printf '1' > "$_f1") &
(sleep 1; printf '2' > "$_f2") &
wait
_rc=$?

_fail=0
if [ "$_rc" -ne 0 ]; then
  printf '%s\n' "FAIL: wait with no args returned $_rc, expected 0" >&2
  _fail=1
fi
if [ ! -f "$_f1" ] || [ ! -f "$_f2" ]; then
  printf '%s\n' "FAIL: wait returned before all children completed" >&2
  _fail=1
fi

rm -f "$_f1" "$_f2"
exit "$_fail"
