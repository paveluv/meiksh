# SHALL-19-05-03-034
# "PWD ... Set by the shell and by the cd utility. In the shell the value shall
#  be initialized from the environment ..."
# Verify PWD is set and updated by cd.

fail=0

# PWD should be set and non-empty
[ -n "$PWD" ] || { printf '%s\n' "FAIL: PWD is empty" >&2; fail=1; }

# PWD should be an absolute path
case "$PWD" in
  /*) ;;
  *) printf '%s\n' "FAIL: PWD not absolute: '$PWD'" >&2; fail=1 ;;
esac

# cd should update PWD
old_pwd=$PWD
cd "$TMPDIR" || { printf '%s\n' "FAIL: cd to TMPDIR failed" >&2; fail=1; }
[ "$PWD" != "$old_pwd" ] || [ "$TMPDIR" = "$old_pwd" ] || { printf '%s\n' "FAIL: PWD not updated after cd" >&2; fail=1; }
cd "$old_pwd" 2>/dev/null

exit "$fail"
