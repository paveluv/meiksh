# SHALL-20-100-03-011
# "If more than one var operand is yet to be processed and one or more output
#  fields are yet to be used, the variable named by the first unprocessed var
#  operand shall be assigned the value of the first unused output field."
# Verifies: when multiple vars and fields remain, fields pair 1:1 with vars.

unset IFS
printf 'a1 b2 c3 d4\n' | {
  read w x y z
  if [ "$w" != "a1" ] || [ "$x" != "b2" ] || [ "$y" != "c3" ] || [ "$z" != "d4" ]; then
    printf '%s\n' "FAIL: w='$w' x='$x' y='$y' z='$z'" >&2; exit 1
  fi
}
[ $? -ne 0 ] && exit 1

exit 0
