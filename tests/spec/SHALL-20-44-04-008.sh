# SHALL-20-44-04-008
# "The following options shall be supported:: Suppress command numbers when listing with -l."
# Verify fc -n suppresses command numbers in -l output.

set -e
HISTFILE="$TMPDIR/hist_$$"
export HISTFILE
HISTSIZE=100
export HISTSIZE

${SHELL} -c '
  HISTFILE="'"$HISTFILE"'"
  HISTSIZE=100
  echo alpha
  echo bravo
  # Get output with numbers
  with_nums=$(fc -l -1 -1)
  # Get output without numbers
  without_nums=$(fc -l -n -1 -1)
  # The numbered output should start with a digit
  case "$with_nums" in
    [0-9]*) ;;
    *) echo "FAIL: fc -l output does not start with number: $with_nums" >&2; exit 1 ;;
  esac
  # The unnumbered output should start with a tab
  case "$without_nums" in
    "	"*) ;;
    *) echo "FAIL: fc -ln output does not start with tab: $without_nums" >&2; exit 1 ;;
  esac
  exit 0
'

rm -f "$HISTFILE"
exit 0
