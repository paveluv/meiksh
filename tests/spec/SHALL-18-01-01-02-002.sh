# SHALL-18-01-01-02-002
# "Independent processes shall be capable of executing independently without
#  either process terminating."
# Verify parent shell and background child both survive concurrently.

tmpf="$TMPDIR/shall_18_02_002_$$"
"${SHELL}" -c '
  printf "%s\n" "bg" > "'"$tmpf"'" &
  bgpid=$!
  wait "$bgpid"
  printf "%s\n" "fg"
'
rc=$?
if [ "$rc" -ne 0 ]; then
  printf '%s\n' "FAIL: shell exited with $rc" >&2
  rm -f "$tmpf"
  exit 1
fi
if [ ! -f "$tmpf" ]; then
  printf '%s\n' "FAIL: background process did not create file" >&2
  exit 1
fi
bg_out=$(cat "$tmpf")
rm -f "$tmpf"
if [ "$bg_out" != "bg" ]; then
  printf '%s\n' "FAIL: background output was '$bg_out', expected 'bg'" >&2
  exit 1
fi

exit 0
