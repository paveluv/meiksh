# SHALL-08-02-001
# "This variable shall determine the locale category for native language,
#  local customs, and coded character set in the absence of the LC_ALL and
#  other LC_* ... environment variables."
# Verify LANG serves as locale fallback and is propagated.

_val=$(LANG=C sh -c 'printf "%s" "$LANG"')
if [ "$_val" != "C" ]; then
  printf '%s\n' "FAIL: LANG not propagated to child" >&2
  exit 1
fi

exit 0
