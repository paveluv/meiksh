# SHALL-20-44-08-007
# "Determine a decimal number representing the limit to the number of previous
#  commands that are accessible. If this variable is unset, an unspecified default
#  greater than or equal to 128 shall be used."
# Verify default HISTSIZE >= 128 when unset.

set -e
HISTFILE="$TMPDIR/hist_default_$$"
export HISTFILE

${MEIKSH:-meiksh} -c '
  HISTFILE="'"$HISTFILE"'"
  unset HISTSIZE
  i=1
  while [ "$i" -le 130 ]; do
    eval "echo dflt_$i"
    i=$((i + 1))
  done
  out=$(fc -l -n)
  count=$(printf "%s\n" "$out" | grep -c "dflt_" || true)
  if [ "$count" -lt 128 ]; then
    echo "FAIL: default HISTSIZE gave only $count entries, expected >= 128" >&2
    exit 1
  fi
  exit 0
'

rm -f "$HISTFILE"
exit 0
