# Test: SHALL-19-07-004
# Obligation: "For the other redirection operators, the word that follows the
#   redirection operator shall be subjected to tilde expansion, parameter
#   expansion, command substitution, arithmetic expansion, and quote removal.
#   Pathname expansion shall not be performed on the word by a non-interactive
#   shell."
# Verifies: redirection words undergo standard expansions (not glob).

f="$TMPDIR/shall_19_07_004_$$"
target="$f"
# Parameter expansion in redirect word
printf '%s\n' "expanded" >"$target"
content=$(cat "$target")
if [ "$content" != "expanded" ]; then
    printf '%s\n' "FAIL: param expansion in redirect word: got '$content'" >&2
    rm -f "$f"
    exit 1
fi

# Command substitution in redirect word
f2="$TMPDIR/shall_19_07_004b_$$"
name="$f2"
printf '%s\n' "cmdsub" >"$(printf '%s' "$name")"
content2=$(cat "$f2")
if [ "$content2" != "cmdsub" ]; then
    printf '%s\n' "FAIL: cmd sub in redirect word: got '$content2'" >&2
    rm -f "$f" "$f2"
    exit 1
fi

rm -f "$f" "$f2"
exit 0
