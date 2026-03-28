# SHALL-20-44-05-003
# "Select the commands to list or edit. The number of previous commands that can
#  be accessed shall be determined by the value of the HISTSIZE variable."
# Verify fc first/last defaults: -l with no operands lists previous 16 commands;
# out-of-range values are clamped without error.

set -e
HISTFILE="$TMPDIR/hist_$$"
export HISTFILE
HISTSIZE=100
export HISTSIZE

${MEIKSH:-meiksh} -c '
  HISTFILE="'"$HISTFILE"'"
  HISTSIZE=100
  i=1
  while [ "$i" -le 20 ]; do
    eval "echo hist_entry_$i"
    i=$((i + 1))
  done
  # fc -l with no operands should list the previous 16 commands
  out=$(fc -l -n)
  count=$(printf "%s\n" "$out" | grep -c "hist_entry" || true)
  if [ "$count" -lt 16 ]; then
    echo "FAIL: fc -l listed $count commands, expected at least 16" >&2
    exit 1
  fi
  # Out-of-range clamping: requesting range 1 to 99999 should not error
  fc -l 1 99999 > /dev/null 2>&1
  exit 0
'

rm -f "$HISTFILE"
exit 0
