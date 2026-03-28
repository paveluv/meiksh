# SHALL-20-44-14-002
# "The following exit values shall be returned:: 0"
# Verify fc -l returns exit status 0 on successful listing.

set -e
HISTFILE="$TMPDIR/hist_$$"
export HISTFILE
HISTSIZE=100
export HISTSIZE

${MEIKSH:-meiksh} -c '
  HISTFILE="'"$HISTFILE"'"
  HISTSIZE=100
  echo exitval_zero
  fc -l -1 -1 > /dev/null
  ret=$?
  if [ "$ret" -ne 0 ]; then
    echo "FAIL: fc -l exit status was $ret, expected 0" >&2
    exit 1
  fi
  exit 0
'

rm -f "$HISTFILE"
exit 0
