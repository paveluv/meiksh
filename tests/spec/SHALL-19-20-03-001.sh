# Test: SHALL-19-20-03-001
# Obligation: "The eval utility shall construct a command string by
#   concatenating arguments together, separating each with a <space> character.
#   The constructed command string shall be tokenized, parsed, and executed by
#   the shell in the current environment."

# eval concatenates args with spaces and executes in current env
eval 'EVAL_VAR=hello'
if [ "$EVAL_VAR" != "hello" ]; then
    printf '%s\n' "FAIL: eval did not execute in current environment" >&2
    exit 1
fi

# eval concatenates multiple arguments with spaces
result=$(eval 'printf' '%s' 'world')
if [ "$result" != "world" ]; then
    printf '%s\n' "FAIL: eval did not concatenate args with spaces, got '$result'" >&2
    exit 1
fi

# eval with multi-arg forms a single command string
eval 'A=one' 'B=two'
if [ "$A" != "one" ] || [ "$B" != "two" ]; then
    printf '%s\n' "FAIL: eval multi-arg did not work" >&2
    exit 1
fi

exit 0
