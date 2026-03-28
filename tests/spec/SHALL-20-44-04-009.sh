# SHALL-20-44-04-009
# "The following options shall be supported:: -r"
# Verify fc supports the -r option (reverse order of commands).

set -e
HISTFILE="$TMPDIR/hist_$$"
export HISTFILE
HISTSIZE=100
export HISTSIZE

${MEIKSH:-meiksh} -c '
  HISTFILE="'"$HISTFILE"'"
  HISTSIZE=100
  echo first_cmd
  echo second_cmd
  echo third_cmd
  normal=$(fc -l -n -3 -1)
  reversed=$(fc -l -n -r -3 -1)
  # Normal order: first_cmd should come before third_cmd
  # Reversed order: third_cmd should come before first_cmd
  first_normal=$(printf "%s\n" "$normal" | head -1 | sed "s/^[[:space:]]*//")
  first_reversed=$(printf "%s\n" "$reversed" | head -1 | sed "s/^[[:space:]]*//")
  case "$first_normal" in
    *first_cmd*) ;;
    *) echo "FAIL: normal first line not first_cmd: $first_normal" >&2; exit 1 ;;
  esac
  case "$first_reversed" in
    *third_cmd*) ;;
    *) echo "FAIL: reversed first line not third_cmd: $first_reversed" >&2; exit 1 ;;
  esac
  exit 0
'

rm -f "$HISTFILE"
exit 0
