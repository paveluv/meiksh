# reviewed: GPT-5.4
# Also covers: SHALL-20-64-03-002
# SHALL-20-64-03-004
# "The sig argument is the value specified by the -s option, -signal_number
#  option, or the -signal_name option, or by SIGTERM, if none of these
#  options is specified."
# Verifies docs/posix/utilities/kill.html#tag_20_64_03:
# default SIGTERM, -s signal_name, -signal_number, and -signal_name.

TMP_BASE=${TMPDIR:-/tmp}
_tmp="$TMP_BASE/kill_03_004.$$"
rm -rf "$_tmp"
mkdir -p "$_tmp" || exit 1

check_signal_form() {
  _sig_label=$1
  shift
  _marker="$_tmp/${_sig_label}"
  rm -f "$_marker"
  trap "printf '%s' $_sig_label >\"$_marker\"" "$_sig_label"
  kill "$@" $$ 2>/dev/null
  if [ ! -f "$_marker" ]; then
    printf '%s\n' "FAIL: signal form '$*' did not deliver $_sig_label" >&2
    rm -rf "$_tmp"
    exit 1
  fi
  _got=$(cat "$_marker")
  if [ "$_got" != "$_sig_label" ]; then
    printf '%s\n' "FAIL: signal form '$*' wrote '$_got', expected '$_sig_label'" >&2
    rm -rf "$_tmp"
    exit 1
  fi
  trap - "$_sig_label"
}

check_signal_form TERM
check_signal_form HUP -s HUP
check_signal_form INT -2
check_signal_form HUP -HUP

rm -rf "$_tmp"
exit 0
