# SHALL-08-02-020
# "If the LANG environment variable is defined and is not null, the value of
#  the LANG environment variable shall be used."
# Verify LANG serves as fallback when LC_ALL and LC_* are unset.

got=$("${MEIKSH:-meiksh}" -c '
  unset LC_ALL LC_CTYPE LC_COLLATE LC_MESSAGES LC_MONETARY LC_NUMERIC LC_TIME
  LANG=POSIX
  export LANG
  printf "%s\n" "$LANG"
')
if [ "$got" != "POSIX" ]; then
  printf '%s\n' "FAIL: LANG not used as fallback, got: $got" >&2
  exit 1
fi

exit 0
