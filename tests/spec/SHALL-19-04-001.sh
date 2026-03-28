# SHALL-19-04-001
# "Reserved words are words that have special meaning to the shell ... The
#  following words shall be recognized as reserved words:
#  ! { } case do done elif else esac fi for if in then until while"
# Verify all required reserved words are recognized.

fail=0

# Test each reserved word by using it in its grammatical context
# if/then/else/elif/fi
eval 'if true; then true; elif true; then true; else true; fi' || { printf '%s\n' "FAIL: if/then/elif/else/fi" >&2; fail=1; }

# case/in/esac
eval 'case x in x) true ;; esac' || { printf '%s\n' "FAIL: case/in/esac" >&2; fail=1; }

# for/in/do/done
eval 'for x in a; do true; done' || { printf '%s\n' "FAIL: for/in/do/done" >&2; fail=1; }

# while/do/done
eval 'while false; do true; done' || { printf '%s\n' "FAIL: while/do/done" >&2; fail=1; }

# until/do/done
eval 'until true; do true; done' || { printf '%s\n' "FAIL: until/do/done" >&2; fail=1; }

# { }
eval '{ true; }' || { printf '%s\n' "FAIL: { }" >&2; fail=1; }

# ! (negation)
eval '! false' || { printf '%s\n' "FAIL: ! negation" >&2; fail=1; }

exit "$fail"
