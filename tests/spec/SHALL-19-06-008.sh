# Test: SHALL-19-06-008
# Obligation: "may be subject to an additional implementation-defined form of
#   expansion that can create multiple fields from a single word. This
#   expansion, if supported, shall be applied before all the other word
#   expansions are applied."
# Verifies: If brace expansion is supported, it runs before other expansions.

# Brace expansion is implementation-defined; test only if supported.
# Check if shell supports brace expansion by testing a simple case.
result=$(eval 'printf "%s\n" {a,b}' 2>/dev/null)
case "$result" in
    a*b*)
        # Brace expansion is supported. Verify it runs before param expansion.
        X=hello
        r=$(eval 'printf "%s\n" {$X,world}' 2>/dev/null)
        # $X should be expanded AFTER brace expansion splits the word
        case "$r" in
            hello*world*) ;; # correct: brace first, then param expansion
            *)
                printf '%s\n' "FAIL: brace expansion did not precede param expansion" >&2
                exit 1
                ;;
        esac
        ;;
    *)
        # Brace expansion not supported; that is conformant. Pass.
        ;;
esac

exit 0
