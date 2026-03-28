# SHALL-18-01-01-04-020
# "Open FIFO. When attempting to create a regular file, and the existing file
#  is a FIFO special file: If the FIFO is not already open for reading, the
#  attempt shall block until the FIFO is opened for reading. Once the FIFO is
#  open for reading, the utility shall open the FIFO for writing and continue
#  with its operation."
# Verify redirection to a FIFO delivers data to a reader.

tmpfifo="$TMPDIR/shall_18_04_020_$$"
rm -f "$tmpfifo"
mkfifo "$tmpfifo"

cat "$tmpfifo" > "$TMPDIR/shall_18_04_020_out_$$" &
reader_pid=$!

"${MEIKSH:-meiksh}" -c 'printf "%s\n" "fifo_data" > "'"$tmpfifo"'"'
wait "$reader_pid"

content=$(cat "$TMPDIR/shall_18_04_020_out_$$")
rm -f "$tmpfifo" "$TMPDIR/shall_18_04_020_out_$$"

if [ "$content" != "fifo_data" ]; then
  printf '%s\n' "FAIL: expected 'fifo_data', got '$content'" >&2
  exit 1
fi

exit 0
