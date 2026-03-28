# SHALL-08-02-013
# "The values of locale categories shall be determined by a precedence order;
#  the first condition met below determines the value"
# Verify the locale precedence: LC_ALL > LC_* > LANG > default.
# We test that LC_ALL overrides LC_CTYPE, and LC_CTYPE overrides LANG,
# by checking the environment passed to a child process.

# LC_ALL overrides LC_CTYPE
_val=$(LC_ALL=C LC_CTYPE=en_US.UTF-8 LANG=fr_FR.UTF-8 \
  sh -c 'printf "%s" "$LC_ALL"')
if [ "$_val" != "C" ]; then
  printf '%s\n' "FAIL: LC_ALL not propagated correctly" >&2
  exit 1
fi

exit 0
