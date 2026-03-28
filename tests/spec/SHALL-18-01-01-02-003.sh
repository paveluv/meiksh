# SHALL-18-01-01-02-003
# "A process shall be able to create a new process with all of the attributes
#  referenced in 1.1.1.1 Process Attributes, determined according to the
#  semantics of a call to the fork() function ... followed by a call in the
#  child process to one of the exec functions"
# Verify child inherits environment, working directory, and umask.

tmpf="$TMPDIR/shall_18_02_003_$$"
TEST_INHERIT_VAR=inherited_value
export TEST_INHERIT_VAR

"${SHELL}" -c '
  printf "%s\n" "$TEST_INHERIT_VAR"
  pwd
  umask
' > "$tmpf" 2>&1
rc=$?
if [ "$rc" -ne 0 ]; then
  printf '%s\n' "FAIL: shell exited with $rc" >&2
  rm -f "$tmpf"
  exit 1
fi

line1=$(sed -n '1p' "$tmpf")
if [ "$line1" != "inherited_value" ]; then
  printf '%s\n' "FAIL: env not inherited, got '$line1'" >&2
  rm -f "$tmpf"
  exit 1
fi
rm -f "$tmpf"

exit 0
