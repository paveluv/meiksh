# SHALL-20-44-14-006
# "Otherwise, the exit status shall be that of the commands executed by fc."
# Verify fc -s returns the exit status of the re-executed command.

set -e
HISTFILE="$TMPDIR/hist_$$"
export HISTFILE
HISTSIZE=100
export HISTSIZE

${MEIKSH:-meiksh} -c '
  HISTFILE="'"$HISTFILE"'"
  HISTSIZE=100
  false
  fc -s false 2>/dev/null
  ret=$?
  if [ "$ret" -eq 0 ]; then
    echo "FAIL: fc -s of false returned 0, expected non-zero" >&2
    exit 1
  fi
  true
  fc -s true 2>/dev/null
  ret=$?
  if [ "$ret" -ne 0 ]; then
    echo "FAIL: fc -s of true returned $ret, expected 0" >&2
    exit 1
  fi
  exit 0
'

rm -f "$HISTFILE"
exit 0
