# SHALL-08-01-005
# "If the variables in the following two sections are present in the environment
#  during the execution of an application or utility, they shall be given the
#  meaning described below."
# Verify the shell honors standard POSIX environment variables (PATH, HOME).

# PATH must be used for command lookup
_dir="$TMPDIR/shall080105.$$"
mkdir -p "$_dir"
printf '#!/bin/sh\nprintf pass\n' > "$_dir/testcmd080105"
chmod +x "$_dir/testcmd080105"

_out=$(PATH="$_dir" testcmd080105 2>&1)
_rc=$?
rm -rf "$_dir"

if [ "$_rc" -ne 0 ] || [ "$_out" != "pass" ]; then
  printf '%s\n' "FAIL: shell did not find command via PATH" >&2
  exit 1
fi

# HOME must be used for tilde expansion
_save="$HOME"
HOME=/tmp
_expanded=$(eval 'printf "%s" ~')
HOME="$_save"

if [ "$_expanded" != "/tmp" ]; then
  printf '%s\n' "FAIL: tilde expansion did not use HOME" >&2
  exit 1
fi

exit 0
