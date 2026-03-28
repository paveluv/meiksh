# SHALL-20-44-08-002
# "The following environment variables shall affect the execution of fc:: FCEDIT"
# Verify fc recognizes FCEDIT for default editor selection.

set -e
HISTFILE="$TMPDIR/hist_$$"
export HISTFILE
HISTSIZE=100
export HISTSIZE

marker="$TMPDIR/fcedit_marker_$$"
fake_editor="$TMPDIR/fcedit_editor_$$"
printf '#!/bin/sh\ntouch "%s"\nexit 0\n' "$marker" > "$fake_editor"
chmod +x "$fake_editor"

${MEIKSH:-meiksh} -c '
  HISTFILE="'"$HISTFILE"'"
  HISTSIZE=100
  FCEDIT="'"$fake_editor"'"
  export FCEDIT
  echo fcedit_test
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
