# SHALL-20-100-14-006
# "The following exit values shall be returned:: >1"
# Verifies: read returns >1 on error (e.g., readonly variable).

readonly ROVAR=x
printf 'newval\n' | {
  readonly ROVAR=x
  read ROVAR 2>/dev/null
  st=$?
  if [ "$st" -le 1 ]; then
    printf '%s\n' "FAIL: exit status=$st expected >1 for readonly" >&2; exit 1
  fi
}
[ $? -ne 0 ] && exit 1

exit 0
