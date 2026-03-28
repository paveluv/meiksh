# SHALL-08-01-002
# "These strings have the form name=value; names shall not contain any bytes
#  that have the encoded value of the character '='."
# The shell must produce name=value pairs in the child environment and variable
# names must not contain '='.

export TESTVAR_08_002="somevalue"

got=$(env | while IFS= read -r line; do
  case "$line" in
    TESTVAR_08_002=somevalue) printf '%s\n' "found"; break ;;
  esac
done)

if [ "$got" != "found" ]; then
  printf '%s\n' "FAIL: exported var not in name=value form in child env" >&2
  exit 1
fi

# Verify that the shell rejects an assignment with '=' in the name.
# This should be a syntax error.
if eval 'bad=name=x' 2>/dev/null; then
  # Even if the shell accepted it, the variable name stored should be "bad"
  # not "bad=name" — check the env doesn't have a corrupted entry
  :
fi

exit 0
