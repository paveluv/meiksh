# Test: SHALL-07-01-001
# Obligation: "The standard utilities in the Shell and Utilities volume of
#   POSIX.1-2024 shall base their behavior on the current locale, as defined
#   in the ENVIRONMENT VARIABLES section for each utility."
# Verifies: The shell respects LC_ALL/LANG for locale-sensitive operations.
#   Tests that the POSIX/C locale produces predictable character class behavior.

# In the POSIX locale, [[:upper:]] should match only A-Z
LC_ALL=C
export LC_ALL

# Verify [[:upper:]] matches uppercase ASCII
case A in
    [[:upper:]]) ;;
    *) echo "FAIL: 'A' should match [[:upper:]] in C locale" >&2; exit 1 ;;
esac

# Verify [[:digit:]] matches digit
case 5 in
    [[:digit:]]) ;;
    *) echo "FAIL: '5' should match [[:digit:]] in C locale" >&2; exit 1 ;;
esac

# Verify [[:lower:]] does not match uppercase
case A in
    [[:lower:]]) echo "FAIL: 'A' should not match [[:lower:]] in C locale" >&2; exit 1 ;;
    *) ;;
esac

exit 0
