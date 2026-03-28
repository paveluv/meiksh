# SHALL-08-02-018
# "If the LC_ALL environment variable is defined and is not null, the value
#  of LC_ALL shall be used."
# Verify LC_ALL overrides LC_CTYPE for locale category determination.

got=$("${SHELL}" -c '
  LC_CTYPE=C
  LC_ALL=POSIX
  export LC_CTYPE LC_ALL
  locale 2>/dev/null | grep LC_CTYPE || printf "%s\n" "$LC_ALL"
')

case "$got" in
  *POSIX*) ;;
  *) printf '%s\n' "FAIL: LC_ALL did not override LC_CTYPE, got: $got" >&2; exit 1 ;;
esac

got2=$("${SHELL}" -c '
  LC_ALL=POSIX
  export LC_ALL
  printf "%s\n" "${LC_ALL}"
')
if [ "$got2" != "POSIX" ]; then
  printf '%s\n' "FAIL: LC_ALL not propagated, got: $got2" >&2
  exit 1
fi

exit 0
