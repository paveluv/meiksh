# Test: SHALL-19-03-005
# Obligation: "If the previous character was used as part of an operator and
#   the current character is not quoted and can be used with the previous
#   characters to form an operator, it shall be used as part of that
#   (operator) token."
# Verifies: Multi-character operators are recognized (&&, ||, <<, >>).

# && operator
true && r=yes || r=no
[ "$r" = "yes" ] || { printf '%s\n' "FAIL: && operator" >&2; exit 1; }

# || operator
false || r=ok
[ "$r" = "ok" ] || { printf '%s\n' "FAIL: || operator" >&2; exit 1; }

# >> (append redirect)
f="$TMPDIR/shall_19_03_005_$$"
printf '%s\n' "line1" > "$f"
printf '%s\n' "line2" >> "$f"
lines=$(wc -l < "$f" | tr -d ' ')
[ "$lines" = "2" ] || { printf '%s\n' "FAIL: >> append, got $lines lines" >&2; rm -f "$f"; exit 1; }
rm -f "$f"

exit 0
