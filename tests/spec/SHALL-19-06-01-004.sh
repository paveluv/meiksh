# Test: SHALL-19-06-01-004
# Obligation: "The pathname that replaces the tilde-prefix shall be treated as
#   if quoted to prevent it being altered by field splitting and pathname
#   expansion; if [...] HOME is set to the null string, this produces an empty
#   field (as opposed to zero fields) as the expanded word."
# Verifies: tilde result is immune to splitting/globbing; HOME="" yields empty field.

count_args() { printf '%s\n' "$#"; }

# Tilde result should not be split even with spaces in HOME
saved="$HOME"
HOME="/path with spaces"
set -f
result=$(printf '%s\n' ~)
if [ "$result" != "/path with spaces" ]; then
    printf '%s\n' "FAIL: ~ with spaces in HOME was altered: '$result'" >&2
    HOME="$saved"
    exit 1
fi
set +f

# HOME="" should produce one empty field, not zero fields
HOME=""
n=$(count_args ~)
if [ "$n" != "1" ]; then
    printf '%s\n' "FAIL: HOME='' ~ produced $n fields, expected 1" >&2
    HOME="$saved"
    exit 1
fi

HOME="$saved"
exit 0
