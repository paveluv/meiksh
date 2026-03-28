# SHALL-08-02-015
# "If the LC_* environment variable ... is defined and is not null, the value
#  of the environment variable shall be used to initialize the category that
#  corresponds to the environment variable."
# Verify individual LC_* variables are propagated to children.

_val=$(unset LC_ALL 2>/dev/null; LC_CTYPE=C sh -c 'printf "%s" "$LC_CTYPE"')
if [ "$_val" != "C" ]; then
  printf '%s\n' "FAIL: LC_CTYPE not propagated to child" >&2
  exit 1
fi

exit 0
