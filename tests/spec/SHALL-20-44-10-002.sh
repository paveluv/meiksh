# SHALL-20-44-10-002
# "If both the -l and -n options are specified, the format of each command shall be:"
# Verify fc -l -n output format is "\t%s\n" (tab + command, no number).

set -e
HISTFILE="$TMPDIR/hist_$$"
export HISTFILE
HISTSIZE=100
export HISTSIZE

${MEIKSH:-meiksh} -c '
  HISTFILE="'"$HISTFILE"'"
  HISTSIZE=100
  echo fmt_ln_test
  out=$(fc -l -n -1 -1)
  # Should start with tab, no number
  case "$out" in
    "	"*fmt_ln_test*) ;;
    *) echo "FAIL: fc -ln format not TAB+CMD: $out" >&2; exit 1 ;;
  esac
  # Should NOT start with a digit
  case "$out" in
    [0-9]*) echo "FAIL: fc -ln still has number prefix: $out" >&2; exit 1 ;;
  esac
  exit 0
'

rm -f "$HISTFILE"
exit 0
