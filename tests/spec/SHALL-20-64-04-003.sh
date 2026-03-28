# SHALL-20-64-04-003
# "The following options shall be supported:: -l"
# Verify: kill -l produces output (list of signal names).

_out=$(kill -l 2>/dev/null)
if [ -z "$_out" ]; then
  printf '%s\n' "FAIL: kill -l produced no output" >&2
  exit 1
fi

# Output must contain at least HUP, INT, KILL, TERM
for _sig in HUP INT KILL TERM; do
  case "$_out" in
    *"$_sig"*) ;;
    *)
      printf '%s\n' "FAIL: kill -l output missing $_sig" >&2
      exit 1
      ;;
  esac
done

exit 0
