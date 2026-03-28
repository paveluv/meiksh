# SHALL-20-44-14-004
# "The following exit values shall be returned:: >0"
# Verify fc returns >0 on error (e.g., invalid option).

set -e
HISTFILE="$TMPDIR/hist_$$"
export HISTFILE
HISTSIZE=100
export HISTSIZE

${MEIKSH:-meiksh} -c '
  HISTFILE="'"$HISTFILE"'"
  HISTSIZE=100
  echo err_test
  fc --invalid-option-that-does-not-exist 2>/dev/null
  ret=$?
  if [ "$ret" -eq 0 ]; then
    echo "FAIL: fc with invalid option returned 0, expected >0" >&2
    exit 1
  fi
  exit 0
'

rm -f "$HISTFILE"
exit 0
