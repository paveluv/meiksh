# SHALL-08-02-016
# "If the LANG environment variable is defined and is not null, the value of
#  the LANG environment variable shall be used."
# Verify LANG is propagated to children.

_val=$(LANG=C sh -c 'printf "%s" "$LANG"')
if [ "$_val" != "C" ]; then
  printf '%s\n' "FAIL: LANG not propagated to child" >&2
  exit 1
fi

exit 0
