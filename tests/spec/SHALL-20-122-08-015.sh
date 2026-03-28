# SHALL-20-122-08-015
# "The following environment variables shall affect the execution of time::
#  Determine the search path that shall be used to locate the utility to be
#  invoked; see XBD 8. Environment Variables."
# Verify time uses PATH to locate the utility.

bindir="$TMPDIR/shall_20_122_08_015_bin_$$"
mkdir -p "$bindir"
cat > "$bindir/myutil" <<'UTIL'
#!/bin/sh
exit 0
UTIL
chmod +x "$bindir/myutil"

got=$("${SHELL}" -c "PATH='$bindir' time myutil" 2>&1) || true
rc=$?
rm -rf "$bindir"

# time should have found and run myutil via PATH
# We mainly verify it did not report "not found"
case "$got" in
  *"not found"*) printf '%s\n' "FAIL: time did not find utility via PATH" >&2; exit 1 ;;
esac

exit 0
