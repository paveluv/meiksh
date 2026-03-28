# SHALL-20-100-11-001
# "The standard error shall be used for diagnostic messages and prompts for
#  continued input."
# Verifies: read writes error diagnostics to stderr, not stdout.

# Invalid option should produce stderr output
out=$(read -Z var </dev/null 2>/dev/null)
if [ -n "$out" ]; then
  printf '%s\n' "FAIL: diagnostic went to stdout: '$out'" >&2; exit 1
fi

# Confirm stderr gets the diagnostic
err=$(read -Z var </dev/null 2>&1)
if [ -z "$err" ]; then
  printf '%s\n' "FAIL: no diagnostic on stderr for bad option" >&2; exit 1
fi

exit 0
