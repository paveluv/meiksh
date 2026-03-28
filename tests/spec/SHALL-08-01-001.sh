# SHALL-08-01-001
# "For a C-language program, an array of strings called the environment shall
#  be made available when a process begins."
# The shell must pass exported variables to child processes via the environment.

TEST_VAR_08_01_001="hello_from_parent"
export TEST_VAR_08_01_001

got=$(env | while IFS= read -r line; do
  case "$line" in
    TEST_VAR_08_01_001=hello_from_parent) printf '%s\n' "found"; break ;;
  esac
done)

if [ "$got" != "found" ]; then
  printf '%s\n' "FAIL: exported variable not visible in child env" >&2
  exit 1
fi

unset TEST_VAR_08_01_001
got2=$(env | while IFS= read -r line; do
  case "$line" in
    TEST_VAR_08_01_001=*) printf '%s\n' "found"; break ;;
  esac
done)

if [ "$got2" = "found" ]; then
  printf '%s\n' "FAIL: unset variable still visible in child env" >&2
  exit 1
fi

exit 0
