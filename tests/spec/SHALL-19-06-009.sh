# Test: SHALL-19-06-009
# Obligation: "This expansion, if supported, shall be applied before all the
#   other word expansions are applied. The other expansions shall then be
#   applied to each field that results from this expansion."
# Verifies: Same ordering constraint as SHALL-19-06-008 (brace before others).

# If brace expansion is supported, verify expansions apply to each result field.
result=$(eval 'printf "%s\n" {a,b}' 2>/dev/null)
case "$result" in
    a*b*)
        A=x B=y
        r=$(eval 'for w in {$A,$B}; do printf "%s\n" "$w"; done' 2>/dev/null)
        case "$r" in
            x*y*) ;; # correct
            *)
                printf '%s\n' "FAIL: expansions not applied to each brace result" >&2
                exit 1
                ;;
        esac
        ;;
    *)
        # Brace expansion not supported; conformant. Pass.
        ;;
esac

exit 0
