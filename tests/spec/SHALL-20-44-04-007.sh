# SHALL-20-44-04-007
# "The following options shall be supported:: -n"
# Verify fc supports the -n option (suppress command numbers with -l).

set -e
HISTFILE="$TMPDIR/hist_$$"
export HISTFILE
HISTSIZE=100
export HISTSIZE

# Seed the history with some commands
${SHELL} -c '
  HISTFILE="'"$HISTFILE"'"
  HISTSIZE=100
  echo cmd_one
  echo cmd_two
  echo cmd_three
  out=$(fc -l -n -2 -1 2>&1) || { echo "FAIL: fc -ln returned non-zero" >&2; exit 1; }
  # With -n, output lines should NOT start with a number
  case "$out" in
    [0-9]*) echo "FAIL: fc -ln output starts with a number: $out" >&2; exit 1 ;;
  esac
  exit 0
'

rm -f "$HISTFILE"
exit 0
