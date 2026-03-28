# SHALL-20-44-08-004
# "The following environment variables shall affect the execution of fc:: HISTFILE"
# Verify fc uses HISTFILE for history storage.

set -e
HISTFILE="$TMPDIR/hist_custom_$$"
export HISTFILE
HISTSIZE=100
export HISTSIZE

${MEIKSH:-meiksh} -c '
  HISTFILE="'"$HISTFILE"'"
  HISTSIZE=100
  echo histfile_test_cmd
  fc -l -1 -1 > /dev/null
  exit 0
'

# HISTFILE should have been created or used
if [ -f "$HISTFILE" ]; then
  rm -f "$HISTFILE"
  exit 0
else
  echo "FAIL: HISTFILE was not created at $HISTFILE" >&2
  exit 1
fi
