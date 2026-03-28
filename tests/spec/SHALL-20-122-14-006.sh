# SHALL-20-122-14-006
# "127 - The utility specified by utility could not be found."
# Verify time exits 127 for a nonexistent utility.

"${SHELL:-sh}" -c 'time -p no_such_utility_xyz_$$' 2>/dev/null
_rc=$?
if [ "$_rc" != "127" ]; then
  printf '%s\n' "FAIL: expected 127, got $_rc" >&2
  exit 1
fi

exit 0
