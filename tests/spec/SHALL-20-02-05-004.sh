# SHALL-20-02-05-004
# "The following operands shall be supported:: alias-name=string"

result=$("$MEIKSH" -c '
  alias ls="echo listing"
  alias ls
')
case "$result" in
  *ls*listing*) ;;
  *)
    printf '%s\n' "FAIL: alias name=string form not supported, got: $result" >&2
    exit 1
    ;;
esac
exit 0
