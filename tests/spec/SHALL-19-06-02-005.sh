# Test: SHALL-19-06-02-005
# Obligation: "word shall be subjected to tilde expansion, parameter expansion,
#   command substitution, arithmetic expansion, and quote removal. If word is
#   not needed, it shall not be expanded."
# Verifies: word in param expansion modifiers is expanded only when needed.

# word IS needed: parameter is unset, :- word should be expanded
unset missing
val="${missing:-$(printf '%s\n' expanded)}"
if [ "$val" != "expanded" ]; then
    printf '%s\n' "FAIL: word not expanded when needed: got '$val'" >&2
    exit 1
fi

# word is NOT needed: parameter is set, :- word should NOT be expanded
present=here
marker="$TMPDIR/shall_19_06_02_005_$$"
val2="${present:-$(touch "$marker")}"
if [ -f "$marker" ]; then
    rm -f "$marker"
    printf '%s\n' "FAIL: word was expanded when not needed (file created)" >&2
    exit 1
fi
rm -f "$marker"

exit 0
