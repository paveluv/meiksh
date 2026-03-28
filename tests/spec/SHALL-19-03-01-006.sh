# SHALL-19-03-01-006
# "The TOKEN shall be subject to alias substitution if ... Either the TOKEN is
#  being considered ... because it follows an alias substitution whose
#  replacement value ended with a <blank> ... or the TOKEN could be parsed as
#  the command name word of a simple command."
# Verify alias expansion occurs only in command-name position, and blank-ending
# aliases trigger checking of the next word.

fail=0

# Normal: alias in command position
alias myecho='printf %s\n'
result=$(eval 'myecho hello')
[ "$result" = "hello" ] || { printf '%s\n' "FAIL: command-position alias failed: '$result'" >&2; fail=1; }

# Blank-ending alias triggers checking of next word
alias noglob='set -f '
alias myword='printf expanded\n'
result=$(eval 'noglob myword')
# myword should be checked for alias expansion because noglob ends with blank
[ "$result" = "expanded" ] || { printf '%s\n' "FAIL: blank-ending alias chain failed: '$result'" >&2; fail=1; }
set +f

unalias myecho noglob myword 2>/dev/null

exit "$fail"
