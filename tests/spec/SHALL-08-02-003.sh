# SHALL-08-02-003
# "This variable shall determine the values for all locale categories. The
#  value of the LC_ALL environment variable has precedence over any of the
#  other environment variables starting with LC_ ... and the LANG
#  environment variable."
# Verify LC_ALL is propagated and overrides other locale variables.

_val=$(LC_ALL=C LANG=en_US.UTF-8 sh -c 'printf "%s" "$LC_ALL"')
if [ "$_val" != "C" ]; then
  printf '%s\n' "FAIL: LC_ALL not propagated to child" >&2
  exit 1
fi

exit 0
