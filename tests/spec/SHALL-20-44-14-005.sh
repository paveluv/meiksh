# SHALL-20-44-14-005
# "The following exit values shall be returned:: An error occurred."
# Verify fc returns >0 when an error occurs.

set -e
HISTFILE="$TMPDIR/hist_$$"
export HISTFILE
HISTSIZE=100
export HISTSIZE

${SHELL} -c '
  HISTFILE="'"$HISTFILE"'"
  HISTSIZE=100
  echo err_desc_test
  fc -Z 2>/dev/null
  ret=$?
  if [ "$ret" -eq 0 ]; then
    echo "FAIL: fc -Z (bad option) returned 0, expected >0" >&2
    exit 1
  fi
  exit 0
'

rm -f "$HISTFILE"
exit 0
