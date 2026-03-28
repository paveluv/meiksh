# SHALL-20-44-10-001
# "When the -l option is used to list commands, the format of each command in the
#  list shall be as follows:"
# Verify fc -l output format is "%d\t%s\n".

set -e
HISTFILE="$TMPDIR/hist_$$"
export HISTFILE
HISTSIZE=100
export HISTSIZE

${SHELL} -c '
  HISTFILE="'"$HISTFILE"'"
  HISTSIZE=100
  echo format_test_cmd
  out=$(fc -l -1 -1)
  # Format: NUMBER<TAB>COMMAND
  case "$out" in
    [0-9]*"	"*format_test_cmd*)  ;;
    *) echo "FAIL: fc -l format not NUMBER<TAB>CMD: $out" >&2; exit 1 ;;
  esac
  exit 0
'

rm -f "$HISTFILE"
exit 0
