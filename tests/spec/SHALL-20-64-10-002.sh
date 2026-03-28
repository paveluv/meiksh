# SHALL-20-64-10-002
# "When the -l option is specified, the symbolic name of each signal shall
#  be written in the following format:"
# Verify: kill -l writes signal names to stdout.

_out=$(kill -l 2>/dev/null)
if [ -z "$_out" ]; then
  printf '%s\n' "FAIL: kill -l produced no output" >&2
  exit 1
fi

# Must contain standard signal names
for _sig in HUP INT QUIT ABRT KILL ALRM TERM; do
  case "$_out" in
    *"$_sig"*) ;;
    *)
      printf '%s\n' "FAIL: kill -l missing signal $_sig" >&2
      exit 1
      ;;
  esac
done

exit 0
