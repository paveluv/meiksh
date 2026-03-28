# SHALL-20-44-05-001
# "The following operands shall be supported:"
# Verify fc accepts the first, last, and old=new operands.

set -e
HISTFILE="$TMPDIR/hist_$$"
export HISTFILE
HISTSIZE=100
export HISTSIZE

${MEIKSH:-meiksh} -c '
  HISTFILE="'"$HISTFILE"'"
  HISTSIZE=100
  echo oper_a
  echo oper_b
  echo oper_c
  # first and last operands
  out=$(fc -l -2 -1 2>&1)
  case "$out" in
    *oper_b*) ;;
    *) echo "FAIL: fc -l with first/last did not list expected cmd: $out" >&2; exit 1 ;;
  esac
  # old=new operand with -s
  out2=$(fc -s oper_c=oper_d echo 2>&1)
  case "$out2" in
    *oper_d*) ;;
    *) echo "FAIL: fc -s old=new did not substitute: $out2" >&2; exit 1 ;;
  esac
  exit 0
'

rm -f "$HISTFILE"
exit 0
