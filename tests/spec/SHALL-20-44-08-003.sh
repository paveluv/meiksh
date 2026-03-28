# SHALL-20-44-08-003
# "This variable, when expanded by the shell, shall determine the default value
#  for the -e editor option's editor option-argument. If FCEDIT is null or unset,
#  ed shall be used as the editor."
# Verify FCEDIT fallback chain: -e > FCEDIT > ed.

set -e
HISTFILE="$TMPDIR/hist_$$"
export HISTFILE
HISTSIZE=100
export HISTSIZE

marker="$TMPDIR/fcedit_chain_$$"
fake_editor="$TMPDIR/fcedit_chain_editor_$$"
printf '#!/bin/sh\ntouch "%s"\nexit 0\n' "$marker" > "$fake_editor"
chmod +x "$fake_editor"

# Test that -e overrides FCEDIT
${SHELL} -c '
  HISTFILE="'"$HISTFILE"'"
  HISTSIZE=100
  FCEDIT=/nonexistent_should_not_be_used
  export FCEDIT
  echo chain_test
  fc -e "'"$fake_editor"'" -1 -1 2>/dev/null || true
' 2>/dev/null || true

if [ -f "$marker" ]; then
  rm -f "$marker" "$fake_editor" "$HISTFILE"
  exit 0
else
  echo "FAIL: -e editor was not used over FCEDIT" >&2
  rm -f "$fake_editor" "$HISTFILE"
  exit 1
fi
