# SHALL-20-100-14-004
# "The following exit values shall be returned:: 1"
# Verifies: read returns exactly 1 on EOF (no trailing delimiter).

printf 'hello' | {
  read var
  st=$?
  if [ "$st" -ne 1 ]; then
    printf '%s\n' "FAIL: exit status=$st expected 1 on EOF" >&2; exit 1
  fi
}
[ $? -ne 0 ] && exit 1

exit 0
