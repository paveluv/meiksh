# Test: SHALL-20-110-06-004
# Obligation: "The standard input shall be used only if one of the following
#   is true: The script executes one or more commands that require input from
#   standard input (such as a read command that does not redirect its input)."
# Verifies: A script file can have its commands read stdin (e.g. read builtin).

cat > "$TMPDIR/readscript.sh" <<'EOF'
read line
printf '%s\n' "$line"
EOF

result=$(printf '%s\n' "hello-from-stdin" | "$MEIKSH" "$TMPDIR/readscript.sh")
if [ "$result" != "hello-from-stdin" ]; then
    printf '%s\n' "FAIL: script's read did not get stdin, got '$result'" >&2
    exit 1
fi

exit 0
