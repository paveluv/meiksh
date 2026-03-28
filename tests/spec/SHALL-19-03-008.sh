# SHALL-19-03-008
# "If the current character is an unquoted '$' or '`', the shell shall identify
#  the start of any candidates for parameter expansion, command substitution, or
#  arithmetic expansion ... The shell shall read sufficient input to determine
#  the end of the unit to be expanded ... shall recursively process them ...
#  shall be included unmodified in the result token ... The token shall not be
#  delimited by the end of the substitution."

fail=0

# Parameter expansion within a word: pre${x}post → preVALpost
x=VAL
result="pre${x}post"
[ "$result" = "preVALpost" ] || { printf '%s\n' "FAIL: pre\${x}post = '$result'" >&2; fail=1; }

# Command substitution within a word: A$(printf BC)D → ABCD
result="A$(printf BC)D"
[ "$result" = "ABCD" ] || { printf '%s\n' "FAIL: A\$(printf BC)D = '$result'" >&2; fail=1; }

# Arithmetic expansion within a word: num$((1+2))end → num3end
result="num$((1+2))end"
[ "$result" = "num3end" ] || { printf '%s\n' "FAIL: num\$((1+2))end = '$result'" >&2; fail=1; }

# Nested: ${x:-$(printf hi)} → VAL  (x is set)
result="${x:-$(printf hi)}"
[ "$result" = "VAL" ] || { printf '%s\n' "FAIL: nested expansion = '$result'" >&2; fail=1; }

# Backtick form: A`printf BC`D → ABCD
result="A`printf BC`D"
[ "$result" = "ABCD" ] || { printf '%s\n' "FAIL: backtick cmd sub = '$result'" >&2; fail=1; }

exit "$fail"
