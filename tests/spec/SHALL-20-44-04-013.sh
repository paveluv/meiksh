# SHALL-20-44-04-013
# "Use the editor named by editor to edit the commands. The editor string is a
#  utility name, subject to search via the PATH variable. The value in the FCEDIT
#  variable shall be used as a default when -e is not specified. If FCEDIT is null
#  or unset, ed shall be used as the editor."
# Verify FCEDIT is used as default editor when -e is not specified.

set -e
HISTFILE="$TMPDIR/hist_$$"
export HISTFILE
HISTSIZE=100
export HISTSIZE

marker="$TMPDIR/fc_editor_marker_$$"

# Create a fake editor that just touches a marker file and exits
fake_editor="$TMPDIR/fc_fake_editor_$$"
printf '#!/bin/sh\ntouch "%s"\nexit 0\n' "$marker" > "$fake_editor"
chmod +x "$fake_editor"

${MEIKSH:-meiksh} -c '
  HISTFILE="'"$HISTFILE"'"
  HISTSIZE=100
  FCEDIT="'"$fake_editor"'"
  export FCEDIT
  echo dummy_cmd
  fc -1 -1 2>/dev/null || true
' 2>/dev/null || true

if [ -f "$marker" ]; then
  rm -f "$marker" "$fake_editor" "$HISTFILE"
  exit 0
else
  echo "FAIL: FCEDIT editor was not invoked" >&2
  rm -f "$fake_editor" "$HISTFILE"
  exit 1
fi
