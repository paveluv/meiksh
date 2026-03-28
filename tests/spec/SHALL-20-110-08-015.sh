# Test: SHALL-20-110-08-015
# Obligation: "If set to a non-empty string value, override the values of
#   all the other internationalization variables."
# Verifies: LC_ALL overrides LC_CTYPE et al. for character classification.

result=$(LC_ALL=C LC_CTYPE=en_US.UTF-8 "$MEIKSH" -c 'case A in [[:upper:]]) printf "ok\n";; *) printf "no\n";; esac')
if [ "$result" != "ok" ]; then
    printf '%s\n' "FAIL: LC_ALL=C did not override LC_CTYPE for char class, got '$result'" >&2
    exit 1
fi

exit 0
