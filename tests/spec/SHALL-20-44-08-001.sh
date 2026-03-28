# SHALL-20-44-08-001
# "The following environment variables shall affect the execution of fc:"
# Verify fc recognizes FCEDIT, HISTFILE, and HISTSIZE environment variables.

set -e
HISTFILE="$TMPDIR/hist_$$"
export HISTFILE
HISTSIZE=50
export HISTSIZE

${MEIKSH:-meiksh} -c '
  HISTFILE="'"$HISTFILE"'"
  HISTSIZE=50
  FCEDIT=cat
  export FCEDIT
  echo env_test_cmd
  # fc -l should work with these env vars set
  out=$(fc -l -1 -1)
  case "$out" in
    *env_test_cmd*) ;;
    *) echo "FAIL: fc did not honor env vars: $out" >&2; exit 1 ;;
  esac
  exit 0
'

rm -f "$HISTFILE"
exit 0
