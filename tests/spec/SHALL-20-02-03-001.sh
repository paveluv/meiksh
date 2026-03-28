# SHALL-20-02-03-001
# "The alias utility shall create or redefine alias definitions or write the values
#  of existing alias definitions to standard output."

result=$("$MEIKSH" -c '
  alias myalias="echo hello"
  alias myalias
')
case "$result" in
  *myalias*hello*) ;;
  *)
    printf '%s\n' "FAIL: alias did not create/display alias, got: $result" >&2
    exit 1
    ;;
esac
exit 0
