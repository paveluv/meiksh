# SHALL-20-110-04-002
# "The option letters derived from the set special built-in shall also be
#  accepted with a leading <plus-sign> ('+') instead of a leading
#  <hyphen-minus> (meaning the reverse case of the option)"
# Verifies: sh accepts set-derived options with + prefix to reverse them.

SH="${MEIKSH:-sh}"

# -e enables errexit, +e disables it
# With +e, a failing command should NOT abort the shell
out=$("$SH" +e -c 'false; printf "%s\n" survived')
if [ "$out" != "survived" ]; then
  printf '%s\n' "FAIL: +e did not disable errexit: '$out'" >&2; exit 1
fi

# -e enables errexit: failing command should abort
out=$("$SH" -e -c 'false; printf "%s\n" survived' 2>/dev/null) || true
if [ "$out" = "survived" ]; then
  printf '%s\n' "FAIL: -e did not enable errexit" >&2; exit 1
fi

exit 0
