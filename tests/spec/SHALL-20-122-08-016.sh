# SHALL-20-122-08-016
# "Determine the search path that shall be used to locate the utility to be
#  invoked; see XBD 8. Environment Variables."
# Verify PATH is used to locate the utility invoked by time.

bindir="$TMPDIR/shall_20_122_08_016_bin_$$"
mkdir -p "$bindir"
cat > "$bindir/testcmd" <<'CMD'
#!/bin/sh
printf "found\n"
CMD
chmod +x "$bindir/testcmd"

got=$("${MEIKSH:-meiksh}" -c "PATH='$bindir' time testcmd" 2>/dev/null)
rm -rf "$bindir"

case "$got" in
  *found*) ;;
  *) printf '%s\n' "FAIL: time did not locate utility via PATH, got: $got" >&2; exit 1 ;;
esac

exit 0
