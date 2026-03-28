# SHALL-20-44-14-001
# "The following exit values shall be returned:"
# Verify fc -l returns 0 on success and >0 on error.

set -e
HISTFILE="$TMPDIR/hist_$$"
export HISTFILE
HISTSIZE=100
export HISTSIZE

${MEIKSH:-meiksh} -c '
  HISTFILE="'"$HISTFILE"'"
  HISTSIZE=100
  echo exit_test
  fc -l -1 -1 > /dev/null
  ret=$?
  if [ "$ret" -ne 0 ]; then
    echo "FAIL: fc -l returned $ret, expected 0" >&2
    exit 1
  fi
  exit 0
'

rm -f "$HISTFILE"
exit 0
