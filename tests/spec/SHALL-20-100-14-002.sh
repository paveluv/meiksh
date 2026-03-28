# SHALL-20-100-14-002
# "The following exit values shall be returned:: 0"
# Verifies: read returns 0 on successful line read with delimiter.

printf 'hello\n' | {
  read var
  st=$?
  if [ "$st" -ne 0 ]; then
    printf '%s\n' "FAIL: exit status=$st expected 0" >&2; exit 1
  fi
}
[ $? -ne 0 ] && exit 1

exit 0
