# SHALL-20-44-04-014
# "(The letter ell.) List the commands rather than invoking an editor on them.
#  The commands shall be written in the sequence indicated by the first and last
#  operands, as affected by -r, with each command preceded by the command number."
# Verify fc -l lists commands with command numbers.

set -e
HISTFILE="$TMPDIR/hist_$$"
export HISTFILE
HISTSIZE=100
export HISTSIZE

${MEIKSH:-meiksh} -c '
  HISTFILE="'"$HISTFILE"'"
  HISTSIZE=100
  echo list_test_alpha
  echo list_test_bravo
  out=$(fc -l -2 -1)
  # Each line should be: NUMBER<TAB>COMMAND
  printf "%s\n" "$out" | while IFS= read -r line; do
    case "$line" in
      [0-9]*"	"*) ;;
      *) echo "FAIL: fc -l line not in NUMBER<TAB>CMD format: $line" >&2; exit 1 ;;
    esac
  done
  exit 0
'

rm -f "$HISTFILE"
exit 0
