# SHALL-18-01-01-02-005
# "A process shall be able to create a new process with all of the attributes
#  referenced in 1.1.1.1 Process Attributes, determined according to the
#  semantics of a call to the fork() function ... followed by a call in the
#  child process to one of the exec functions"
# Verify child process inherits file descriptors for redirection.

tmpf="$TMPDIR/shall_18_02_005_$$"
"${SHELL}" -c 'printf "%s\n" "fd_test"' > "$tmpf"
content=$(cat "$tmpf")
rm -f "$tmpf"
if [ "$content" != "fd_test" ]; then
  printf '%s\n' "FAIL: fd inheritance for redirection failed, got '$content'" >&2
  exit 1
fi

exit 0
