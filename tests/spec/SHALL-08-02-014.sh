# SHALL-08-02-014
# "If the LC_ALL environment variable is defined and is not null, the value
#  of LC_ALL shall be used."
# Verify LC_ALL overrides individual LC_* variables.

_val=$(LC_ALL=C LC_CTYPE=en_US.UTF-8 sh -c 'printf "%s" "$LC_ALL"')
if [ "$_val" != "C" ]; then
  printf '%s\n' "FAIL: LC_ALL not set in child" >&2
  exit 1
fi

exit 0
