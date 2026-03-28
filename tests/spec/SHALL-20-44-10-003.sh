# SHALL-20-44-10-003
# "If the <command> consists of more than one line, the lines after the first
#  shall be displayed as:"
# Verify multi-line commands have continuation lines indented with tab.

set -e
HISTFILE="$TMPDIR/hist_$$"
export HISTFILE
HISTSIZE=100
export HISTSIZE

${MEIKSH:-meiksh} -c '
  HISTFILE="'"$HISTFILE"'"
  HISTSIZE=100
  for x in a b; do echo "$x"; done
  out=$(fc -l -1 -1)
  # Multi-line command: continuation lines should start with tab
  linecount=$(printf "%s\n" "$out" | wc -l | tr -d " ")
  if [ "$linecount" -gt 1 ]; then
    # Check continuation lines start with tab
    printf "%s\n" "$out" | tail -n +2 | while IFS= read -r line; do
      case "$line" in
        "	"*) ;;
        *) echo "FAIL: continuation line not tab-indented: $line" >&2; exit 1 ;;
      esac
    done
  fi
  exit 0
'

rm -f "$HISTFILE"
exit 0
