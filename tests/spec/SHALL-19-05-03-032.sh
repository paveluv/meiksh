# SHALL-19-05-03-032
# "PS4 ... When an execution trace (set -x) is being performed, before each
#  line in the execution trace, the value of this variable shall be subjected to
#  parameter expansion ... The default value shall be \"+ \"."
# Verify PS4 is used as trace prefix with set -x.

fail=0

# Default PS4 should be "+ "
result=$("${MEIKSH:-sh}" -c 'set -x; true' 2>&1)
case "$result" in
  *"+ true"*) ;;
  *"+ "*)  ;;
  *) printf '%s\n' "FAIL: default PS4 trace missing '+ ': '$result'" >&2; fail=1 ;;
esac

# Custom PS4
result=$("${MEIKSH:-sh}" -c 'PS4="TRACE: "; set -x; true' 2>&1)
case "$result" in
  *"TRACE: "*)  ;;
  *) printf '%s\n' "FAIL: custom PS4 not used: '$result'" >&2; fail=1 ;;
esac

exit "$fail"
