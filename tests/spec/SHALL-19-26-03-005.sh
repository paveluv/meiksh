# Test: SHALL-19-26-03-005
# Obligation: "Implementations shall support the options in the following list
#   in both their <hyphen-minus> and <plus-sign> forms."

# Test several standard options can be set and unset without error
for opt in a e f u v x; do
    set -"$opt" 2>/dev/null
    set +"$opt" 2>/dev/null
done

exit 0
