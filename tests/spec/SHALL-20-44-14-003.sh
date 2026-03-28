# SHALL-20-44-14-003
# "The following exit values shall be returned:: Successful completion of the listing."
# Verify fc -l exits 0 when listing completes successfully.

set -e
HISTFILE="$TMPDIR/hist_$$"
export HISTFILE
HISTSIZE=100
export HISTSIZE

${MEIKSH:-meiksh} -c '
  HISTFILE="'"$HISTFILE"'"
  HISTSIZE=100
  echo listing_success
  fc -l -1 -1 > /dev/null 2>&1
  ret=$?
  if [ "$ret" -ne 0 ]; then
    echo "FAIL: fc -l did not exit 0 on success: $ret" >&2
    exit 1
  fi
  exit 0
'

rm -f "$HISTFILE"
exit 0
