# Test: SHALL-19-10-01-007
# Obligation: "The token identifier of the recognized reserved word, for rule 1"
# Verifies: Reserved words recognized in command position.

# 'if' recognized as reserved word
if true; then
    :
fi

# 'while' recognized as reserved word
ran=no
while false; do ran=yes; done

# 'for' recognized as reserved word
for _x in a; do :; done

# 'case' recognized as reserved word
case x in x) ;; esac

# If we got here, all reserved words were recognized
exit 0
