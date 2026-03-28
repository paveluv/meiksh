# SHALL-08-03-009
# "This variable shall represent a pathname of a directory made available for
#  programs that need a place to create temporary files."
# Verify TMPDIR is propagated and usable.

if [ -z "$TMPDIR" ]; then
  printf '%s\n' "FAIL: TMPDIR is not set" >&2
  exit 1
fi

if [ ! -d "$TMPDIR" ]; then
  printf '%s\n' "FAIL: TMPDIR does not point to a directory" >&2
  exit 1
fi

# Verify propagation
_val=$(TMPDIR="$TMPDIR" sh -c 'printf "%s" "$TMPDIR"')
if [ "$_val" != "$TMPDIR" ]; then
  printf '%s\n' "FAIL: TMPDIR not propagated to child" >&2
  exit 1
fi

exit 0
