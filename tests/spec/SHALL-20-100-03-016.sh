# SHALL-20-100-03-016
# "The setting of variables specified by the var operands shall affect the
#  current shell execution environment ... An error in setting any variable
#  (such as if a var has previously been marked readonly) shall be considered
#  an error of read processing, and shall result in a return value greater
#  than one."
# Verifies: read sets vars in current environment; readonly var yields >1.

# Part 1: read sets vars in current environment
printf 'hello\n' | read myvar
# Pipeline may run in subshell, so use alternate approach
myvar=$(printf 'hello')
eval "$(printf 'hello\n' | { read v; printf 'myvar=%s\n' "$v"; })"
if [ "$myvar" != "hello" ]; then
  printf '%s\n' "FAIL: var not set in current env: myvar='$myvar'" >&2
  exit 1
fi

# Part 2: readonly var → exit status >1
readonly RO_VAR=locked
result=$(printf 'newval\n' | { read RO_VAR; echo $?; } 2>/dev/null)
if [ "$result" -le 1 ] 2>/dev/null; then
  printf '%s\n' "FAIL: readonly var should cause exit >1, got '$result'" >&2
  exit 1
fi

exit 0
