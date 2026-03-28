# Test: SHALL-20-110-06-005
# Obligation: "When the shell is using standard input and it invokes a command
#   that also uses standard input, the shell shall ensure that the standard
#   input file pointer points directly after the command it has read when the
#   command begins execution."
# Verifies: The shell does not consume stdin beyond the current command, so
#   a child 'read' sees the next line.

# Use a script file whose commands read from stdin via pipe
cat > "$TMPDIR/stdinpos.sh" <<'EOF'
read a
read b
printf '%s\n' "$a:$b"
EOF
result=$(printf 'line1\nline2\n' | "$MEIKSH" "$TMPDIR/stdinpos.sh")
if [ "$result" != "line1:line2" ]; then
    printf '%s\n' "FAIL: stdin file pointer not positioned correctly, got '$result'" >&2
    exit 1
fi

exit 0
