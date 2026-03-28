# reviewed: GPT-5.4
# Also covers: SHALL-20-110-08-041
# Test: SHALL-20-110-08-033
# Obligation: "This variable shall represent an absolute pathname of the
#   current working directory. Assignments to this variable may be ignored."
# Verifies: PWD holds an absolute path of the cwd after cd.

SH="${MEIKSH:-${SHELL:-sh}}"

result=$("$SH" -c 'cd /tmp && printf "%s\n" "$PWD"')
case "$result" in
    /tmp|/tmp/) ;;  # /tmp may have trailing slash on some systems
    /private/tmp|/private/tmp/) ;;  # macOS resolves /tmp -> /private/tmp
    *)
        printf '%s\n' "FAIL: PWD not absolute path to cwd, got '$result'" >&2
        exit 1
        ;;
esac

# Verify it starts with /
case "$result" in
    /*) ;;
    *)
        printf '%s\n' "FAIL: PWD is not absolute (no leading /), got '$result'" >&2
        exit 1
        ;;
esac

exit 0
