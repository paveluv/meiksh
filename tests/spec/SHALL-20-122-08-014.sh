# SHALL-20-122-08-014
# "PATH: Determine the search path that shall be used to locate the utility to
#  be invoked."
# Verify time uses PATH to locate the utility.

_dir="${TMPDIR}/test_time_path_$$"
mkdir -p "$_dir"
cat > "$_dir/myutil" <<'SCRIPT'
printf '%s\n' "found-via-path"
SCRIPT
chmod +x "$_dir/myutil"

_out=$(PATH="$_dir" "${SHELL:-sh}" -c 'time -p myutil' 2>/dev/null)
rm -rf "$_dir"

if [ "$_out" != "found-via-path" ]; then
  printf '%s\n' "FAIL: time did not find utility via PATH, got '$_out'" >&2
  exit 1
fi

exit 0
