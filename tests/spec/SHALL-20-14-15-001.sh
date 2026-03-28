# SHALL-20-14-15-001
# "The working directory shall remain unchanged."
# Verify working directory is unchanged when cd fails.

_orig="$PWD"
cd /nonexistent_dir_$$ 2>/dev/null
_after="$PWD"

if [ "$_orig" != "$_after" ]; then
  printf '%s\n' "FAIL: PWD changed on cd error: '$_orig' -> '$_after'" >&2
  exit 1
fi

exit 0
