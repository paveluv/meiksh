# Test: SHALL-19-14-02-004
# Obligation: "The <asterisk> ('*') is a pattern that shall match any string,
#   including the null string."
# (Duplicate of SHALL-19-14-02-001)
# Verifies: * matches any string including empty.

m=no; case "" in *) m=yes ;; esac
if [ "$m" != "yes" ]; then printf '%s\n' "FAIL: * no match empty" >&2; exit 1; fi
m=no; case "xyz" in *) m=yes ;; esac
if [ "$m" != "yes" ]; then printf '%s\n' "FAIL: * no match xyz" >&2; exit 1; fi
exit 0
