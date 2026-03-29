# Test: export and readonly Built-ins
# Target: tests/matrix/tests/export_readonly.sh
#
# Tests POSIX requirements for export -p, readonly -p output formatting,
# and set -o allexport.

. "$MATRIX_DIR/lib.sh"

# ==============================================================================
# export -p output format
# ==============================================================================
# REQUIREMENT: SHALL-V3CHAP02-1015-DUP505:
# export -p output format: "export %s=%s\n" for set names,
# "export %s\n" for unset.
# REQUIREMENT: SHALL-V3CHAP02-1016-DUP506:
# Shell shall format export -p output with proper quoting, suitable for
# reinput to the shell.

_out=$($TARGET_SHELL -c 'MYEXPORTVAR=hello; export MYEXPORTVAR; export -p' | grep MYEXPORTVAR)
case "$_out" in
    *export*MYEXPORTVAR*hello*) pass ;;
    *) fail "export -p format wrong: '$_out'" ;;
esac

# Verify export -p output is suitable for reinput
_out2=$($TARGET_SHELL -c '
    TESTV="has spaces"
    export TESTV
    export -p | grep TESTV
')
case "$_out2" in
    *export*TESTV*) pass ;;
    *) fail "export -p didn't handle spaces properly: '$_out2'" ;;
esac

# ==============================================================================
# set -o allexport / set -a
# ==============================================================================
# REQUIREMENT: SHALL-V3CHAP02-1018-DUP540:
# set -o allexport shall be equivalent to -a.

_out3=$($TARGET_SHELL -c '
    set -o allexport
    ALLEXP_VAR=allexp_val
    env | grep ALLEXP_VAR
')
case "$_out3" in
    *ALLEXP_VAR=allexp_val*) pass ;;
    *) fail "set -o allexport didn't export: '$_out3'" ;;
esac

# ==============================================================================
# readonly -p output format
# ==============================================================================
# REQUIREMENT: SHALL-V3CHAP02-1017-DUP513:
# readonly -p output shall be formatted with quoting suitable for reinput.

_out4=$($TARGET_SHELL -c 'ROVAR=roval; readonly ROVAR; readonly -p' | grep ROVAR)
case "$_out4" in
    *readonly*ROVAR*roval*) pass ;;
    *) fail "readonly -p format wrong: '$_out4'" ;;
esac

report
