# Test: SHALL-19-14-02-003
# Obligation: "each <asterisk> shall match a string of zero or more characters,
#   matching the greatest possible number of characters that still allows the
#   remainder of the pattern to match the string."
# Verifies: * uses greedy matching in patterns.

# Greedy matching: ${var%%pattern} vs ${var%pattern} exposes greedy/non-greedy
# but for case patterns, greedy is the default.
# Test: a*b against aXXbYYb - should match (the * is greedy, takes XXbYY)
case "aXXbYYb" in a*b) ;; *) printf '%s\n' "FAIL: a*b did not match aXXbYYb" >&2; exit 1 ;; esac

# *x* should match strings containing x
case "helloXworld" in *X*) ;; *) printf '%s\n' "FAIL: *X* did not match helloXworld" >&2; exit 1 ;; esac
case "hello" in *X*) printf '%s\n' "FAIL: *X* matched hello (no X)" >&2; exit 1 ;; *) ;; esac

# Demonstrate greedy with parameter expansion
var="file.tar.gz"
# Longest prefix removal (##) uses greedy *
result="${var##*.}"
if [ "$result" != "gz" ]; then
    printf '%s\n' "FAIL: greedy ## gave [$result] expected [gz]" >&2
    exit 1
fi
# Shortest prefix removal (#) gives different result
result="${var#*.}"
if [ "$result" != "tar.gz" ]; then
    printf '%s\n' "FAIL: non-greedy # gave [$result] expected [tar.gz]" >&2
    exit 1
fi

exit 0
