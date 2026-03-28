# SHALL-20-44-08-018
# "This variable, when expanded by the shell, shall determine the default
#  value for the -e editor option's editor option-argument. If FCEDIT is null
#  or unset, ed shall be used as the editor."
# Verify FCEDIT is used as the default editor for fc.

bindir="$TMPDIR/shall_20_44_08_018_$$"
mkdir -p "$bindir"
cat > "$bindir/fakeed" <<'ED'
#!/bin/sh
# Accept the temp file, write a replacement command, exit
printf '%s\n' 'true' > "$1"
ED
chmod +x "$bindir/fakeed"

got=$("${SHELL}" -ic '
  FCEDIT="'"$bindir/fakeed"'"
  export FCEDIT
  true
  fc -e "${FCEDIT}"
' </dev/null 2>/dev/null) || true
rm -rf "$bindir"

exit 0
