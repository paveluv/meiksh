# SHALL-20-53-11-005
# "The invoking program name shall be identified in the message. The invoking
#  program name shall be the value of the shell special parameter 0 at the
#  time the getopts utility is invoked."
# Verify getopts error messages include $0.

tmpf="$TMPDIR/shall_20_53_11_005_$$.sh"
cat > "$tmpf" <<'SCRIPT'
OPTIND=1
getopts "ab" opt -z
SCRIPT

err=$("${SHELL}" "$tmpf" 2>&1 >/dev/null)
rm -f "$tmpf"

# The error message should contain the script name (the value of $0)
case "$err" in
  *shall_20_53_11_005*) ;;
  *) printf '%s\n' "FAIL: getopts error should identify \$0, got: $err" >&2; exit 1 ;;
esac

exit 0
