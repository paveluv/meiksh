# SHALL-08-03-007
# "This variable shall represent an absolute pathname of the current working
#  directory. It shall not contain any components that are dot or dot-dot.
#  The value is set by the cd utility, and by the sh utility during
#  initialization."
# Verify PWD is set to an absolute path without . or .. components,
# and that cd updates PWD.

# PWD should be set
if [ -z "$PWD" ]; then
  printf '%s\n' "FAIL: PWD is not set" >&2
  exit 1
fi

# PWD should be absolute (starts with /)
case "$PWD" in
  /*) ;;
  *)
    printf '%s\n' "FAIL: PWD is not absolute: $PWD" >&2
    exit 1
    ;;
esac

# PWD should not contain /. or /.. components
case "$PWD" in
  */./* | */../* | */. | */..)
    printf '%s\n' "FAIL: PWD contains . or .. component: $PWD" >&2
    exit 1
    ;;
esac

# cd should update PWD
_dir="$TMPDIR/shall080307.$$"
mkdir -p "$_dir"
cd "$_dir"
if [ "$PWD" != "$_dir" ]; then
  rm -rf "$_dir"
  printf '%s\n' "FAIL: cd did not update PWD" >&2
  exit 1
fi

rm -rf "$_dir"
exit 0
