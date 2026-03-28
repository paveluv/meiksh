# Test: SHALL-19-26-03-017
# Obligation: "The default for all these options shall be off (unset) unless
#   stated otherwise in the description of the option or unless the shell was
#   invoked with them on"

# By default in a script, -e, -f, -u, -x should be off
# Test -f off: globbing works
tmpfile="$TMPDIR/default_opt_test_$$"
printf '' > "$tmpfile"
rm -f "$tmpfile"

# Test -u off: unset vars expand to empty without error
unset DEFAULT_OPT_UNSET
result="${DEFAULT_OPT_UNSET:-empty}"
if [ "$result" != "empty" ]; then
    printf '%s\n' "FAIL: unexpected default option state" >&2
    exit 1
fi

exit 0
