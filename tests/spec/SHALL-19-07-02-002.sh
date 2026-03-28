# Test: SHALL-19-07-02-002
# Obligation: "Output redirection using the '>' format shall fail if the
#   noclobber option is set (see the description of set -C) and the file named
#   by the expansion of word exists and is [...] a regular file."
# Verifies: set -C prevents clobbering; >| overrides.

f="$TMPDIR/shall_19_07_02_002_$$"
printf '%s\n' "original" >"$f"

# Enable noclobber
set -C
result=$(eval 'printf "%s\n" overwrite >"$f"' 2>&1) && {
    set +C
    printf '%s\n' "FAIL: > did not fail with noclobber on existing file" >&2
    rm -f "$f"
    exit 1
}

# Verify original content preserved
content=$(cat "$f")
if [ "$content" != "original" ]; then
    set +C
    printf '%s\n' "FAIL: file was clobbered despite noclobber" >&2
    rm -f "$f"
    exit 1
fi

# >| should override noclobber
printf '%s\n' "clobbered" >|"$f"
content2=$(cat "$f")
set +C
if [ "$content2" != "clobbered" ]; then
    printf '%s\n' "FAIL: >| did not override noclobber: got '$content2'" >&2
    rm -f "$f"
    exit 1
fi

rm -f "$f"
exit 0
