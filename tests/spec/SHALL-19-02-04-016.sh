# Test: SHALL-19-02-04-016
# Obligation: "The behavior of an unescaped <backslash> immediately followed
#   by any other character, including <newline>, is unspecified."
# Verifies: This is an unspecified-behavior clause; no test can fail.
# We simply verify the shell does not crash on an unrecognized escape.

# \z is unspecified — just make sure the shell doesn't crash
r=$'\z' 2>/dev/null
# No assertion on the value; just verify we get here alive
exit 0
