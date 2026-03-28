# SHALL-18-01-01-04-025
# "Open FIFO. When attempting to create a regular file, and the existing file
#  is a FIFO special file: If the FIFO is not already open for reading, the
#  attempt shall block until the FIFO is opened for reading."
# (Duplicate of 04-020) Verify FIFO redirection delivers data.

tmpfifo="$TMPDIR/shall_18_04_025_$$"
rm -f "$tmpfifo"
mkfifo "$tmpfifo"

cat "$tmpfifo" > "$TMPDIR/shall_18_04_025_out_$$" &
reader_pid=$!

"${MEIKSH:-meiksh}" -c 'printf "%s\n" "fifo_ok" > "'"$tmpfifo"'"'
wait "$reader_pid"

content=$(cat "$TMPDIR/shall_18_04_025_out_$$")
rm -f "$tmpfifo" "$TMPDIR/shall_18_04_025_out_$$"

if [ "$content" != "fifo_ok" ]; then
  printf '%s\n' "FAIL: expected 'fifo_ok', got '$content'" >&2
  exit 1
fi

exit 0
