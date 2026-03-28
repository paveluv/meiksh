# SHALL-08-02-022
# "If the locale value is \"C\" or \"POSIX\", the POSIX locale shall be used
#  and the standard utilities behave in accordance with the rules in 7.2
#  POSIX Locale for the associated category."
# Verify POSIX locale bracket expression behavior: [a-z] matches only
# lowercase ASCII when LC_ALL=C.

_match=yes
_result=$(LC_ALL=C sh -c 'case A in [a-z]) printf match;; *) printf nomatch;; esac')
if [ "$_result" != "nomatch" ]; then
  printf '%s\n' "FAIL: [a-z] matched 'A' in POSIX locale" >&2
  exit 1
fi

_result=$(LC_ALL=C sh -c 'case a in [a-z]) printf match;; *) printf nomatch;; esac')
if [ "$_result" != "match" ]; then
  printf '%s\n' "FAIL: [a-z] did not match 'a' in POSIX locale" >&2
  exit 1
fi

exit 0
