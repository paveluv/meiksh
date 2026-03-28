# SHALL-08-02-019
# "If the LC_* environment variable (LC_COLLATE, LC_CTYPE, LC_MESSAGES,
#  LC_MONETARY, LC_NUMERIC, LC_TIME) is defined and is not null, the value
#  of the environment variable shall be used to initialize the category that
#  corresponds to the environment variable."
# Verify individual LC_* variables are used when LC_ALL is unset.

got=$("${SHELL}" -c '
  unset LC_ALL
  LC_CTYPE=POSIX
  export LC_CTYPE
  printf "%s\n" "$LC_CTYPE"
')
if [ "$got" != "POSIX" ]; then
  printf '%s\n' "FAIL: LC_CTYPE not used, got: $got" >&2
  exit 1
fi

got2=$("${SHELL}" -c '
  unset LC_ALL
  LC_MESSAGES=POSIX
  export LC_MESSAGES
  printf "%s\n" "$LC_MESSAGES"
')
if [ "$got2" != "POSIX" ]; then
  printf '%s\n' "FAIL: LC_MESSAGES not used, got: $got2" >&2
  exit 1
fi

exit 0
