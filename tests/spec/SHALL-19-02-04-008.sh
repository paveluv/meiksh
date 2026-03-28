# Test: SHALL-19-02-04-008
# Obligation: "\f yields a <form-feed> character."
# Verifies: \f in $'...' produces FF (0x0C).

r=$'\f'
expected=$(printf '\014')
[ "$r" = "$expected" ] || { printf '%s\n' "FAIL: \\f should be FF (0x0C)" >&2; exit 1; }

exit 0
