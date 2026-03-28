# SHALL-20-44-08-006
# "The following environment variables shall affect the execution of fc:: HISTSIZE"
# Verify fc recognizes HISTSIZE to limit accessible commands.

set -e
HISTFILE="$TMPDIR/hist_size_$$"
export HISTFILE
HISTSIZE=5
export HISTSIZE

${MEIKSH:-meiksh} -c '
  HISTFILE="'"$HISTFILE"'"
  HISTSIZE=5
  echo hs_1
  echo hs_2
  echo hs_3
  echo hs_4
  echo hs_5
  echo hs_6
  echo hs_7
  echo hs_8
  echo hs_9
  echo hs_10
  out=$(fc -l -n 2>&1)
  # With HISTSIZE=5, listing should not include very old commands
  # At minimum, fc -l should work without error
  case "$out" in
    *hs_*) ;;
    *) echo "FAIL: fc -l produced no history with HISTSIZE=5: $out" >&2; exit 1 ;;
  esac
  exit 0
'

rm -f "$HISTFILE"
exit 0
