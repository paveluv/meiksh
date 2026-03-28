# Test: SHALL-19-06-02-010
# Obligation: "use of the <colon> in the format shall result in a test for a
#   parameter that is unset or null; omission of the <colon> shall result in a
#   test for a parameter that is only unset."
# Verifies: colon vs no-colon behavior in param expansion modifiers.

# With colon: tests unset OR null
nullvar=""
result_colon="${nullvar:-default}"
if [ "$result_colon" != "default" ]; then
    printf '%s\n' "FAIL: \${nullvar:-default} gave '$result_colon' (colon should trigger on null)" >&2
    exit 1
fi

# Without colon: tests ONLY unset
result_nocolon="${nullvar-default}"
if [ "$result_nocolon" != "" ]; then
    printf '%s\n' "FAIL: \${nullvar-default} gave '$result_nocolon' (no-colon should not trigger on null)" >&2
    exit 1
fi

# Without colon on truly unset var
unset truly_unset
result_unset="${truly_unset-fallback}"
if [ "$result_unset" != "fallback" ]; then
    printf '%s\n' "FAIL: \${truly_unset-fallback} gave '$result_unset'" >&2
    exit 1
fi

# :+ vs + on null value
nullvar2=""
result_colonplus="${nullvar2:+alt}"
result_plus="${nullvar2+alt}"
if [ "$result_colonplus" != "" ]; then
    printf '%s\n' "FAIL: \${nullvar2:+alt} gave '$result_colonplus' (should be empty)" >&2
    exit 1
fi
if [ "$result_plus" != "alt" ]; then
    printf '%s\n' "FAIL: \${nullvar2+alt} gave '$result_plus' (should be 'alt')" >&2
    exit 1
fi

exit 0
