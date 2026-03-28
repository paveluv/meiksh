# Test: SHALL-19-06-04-009
# Obligation: "If the shell variable x contains a value that forms a valid
#   integer constant, optionally including a leading <plus-sign> or
#   <hyphen-minus>, then the arithmetic expansions '$(( x ))' and '$(( $x ))'
#   shall return the same value."
# Verifies: bare variable name in arithmetic = $var expansion.

x=42
r1=$((x))
r2=$(($x))
if [ "$r1" != "$r2" ]; then
    printf '%s\n' "FAIL: \$((x))=$r1 != \$((\$x))=$r2 for x=42" >&2
    exit 1
fi

# With leading minus
x=-7
r3=$((x))
r4=$(($x))
if [ "$r3" != "$r4" ]; then
    printf '%s\n' "FAIL: \$((x))=$r3 != \$((\$x))=$r4 for x=-7" >&2
    exit 1
fi

# With leading plus
x=+10
r5=$((x))
r6=$(($x))
if [ "$r5" != "$r6" ]; then
    printf '%s\n' "FAIL: \$((x))=$r5 != \$((\$x))=$r6 for x=+10" >&2
    exit 1
fi

exit 0
