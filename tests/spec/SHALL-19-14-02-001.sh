# Test: SHALL-19-14-02-001
# Obligation: "The <asterisk> ('*') is a pattern that shall match any string,
#   including the null string."
# Verifies: * matches empty string and arbitrary strings.

case "" in *) ;; esac  # must match
case "anything" in *) ;; esac  # must match
case "hello world" in *) ;; esac  # must match

# * alone in case matches everything
matched=no
case "test" in *) matched=yes ;; esac
if [ "$matched" != "yes" ]; then
    printf '%s\n' "FAIL: * did not match 'test'" >&2
    exit 1
fi

# * matches null string
matched=no
case "" in *) matched=yes ;; esac
if [ "$matched" != "yes" ]; then
    printf '%s\n' "FAIL: * did not match empty string" >&2
    exit 1
fi

exit 0
