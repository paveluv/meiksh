# Test: SHALL-19-02-04-019
# Obligation: "If a \e or \cX escape sequence specifies a character that does
#   not have an encoding in the locale ... implementations shall not replace
#   an unsupported character with bytes that do not form valid characters."
# Verifies: \e and \cA produce valid characters (in typical UTF-8/ASCII locale).

# In UTF-8/ASCII locales, ESC (0x1B) and control chars have valid encodings
r=$'\e'
if [ -z "$r" ]; then
    printf '%s\n' "FAIL: \\e produced empty string" >&2; exit 1
fi

r=$'\cA'
if [ -z "$r" ]; then
    printf '%s\n' "FAIL: \\cA produced empty string" >&2; exit 1
fi

exit 0
