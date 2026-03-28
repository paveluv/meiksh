# Test: SHALL-19-26-03-009
# Obligation: "When this option is on, when any command fails ... the shell
#   immediately shall exit ... The -e setting shall be ignored when executing
#   the compound list following the while, until, if, or elif reserved word,
#   a pipeline beginning with the ! reserved word, or any command of an
#   AND-OR list other than the last."

# set -e causes exit on failure (test in subshell)
result=$(set -e; false; printf '%s' "not_reached")
if [ "$result" = "not_reached" ]; then
    printf '%s\n' "FAIL: set -e did not cause exit on false" >&2
    exit 1
fi

# -e ignored in if condition
result=$(set -e; if false; then :; fi; printf '%s' "ok")
if [ "$result" != "ok" ]; then
    printf '%s\n' "FAIL: set -e should be ignored in if condition" >&2
    exit 1
fi

# -e ignored in while condition
result=$(set -e; while false; do :; done; printf '%s' "ok")
if [ "$result" != "ok" ]; then
    printf '%s\n' "FAIL: set -e should be ignored in while condition" >&2
    exit 1
fi

# -e ignored in ! pipeline
result=$(set -e; ! true; printf '%s' "ok")
if [ "$result" != "ok" ]; then
    printf '%s\n' "FAIL: set -e should be ignored with ! pipeline" >&2
    exit 1
fi

# -e ignored in AND-OR list (non-last command)
result=$(set -e; false || true; printf '%s' "ok")
if [ "$result" != "ok" ]; then
    printf '%s\n' "FAIL: set -e should be ignored in OR-list non-last cmd" >&2
    exit 1
fi

exit 0
