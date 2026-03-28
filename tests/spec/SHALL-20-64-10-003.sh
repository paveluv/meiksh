# SHALL-20-64-10-003
# "where the <signal_name> is in uppercase, without the SIG prefix, and
#  the <separator> shall be either a <newline> or a <space>. For the last
#  signal written, <separator> shall be a <newline>."
# Verify: kill -l output format — uppercase, no SIG prefix, ends with newline.

_out=$(kill -l 2>/dev/null)

# Check no SIG prefix appears
case "$_out" in
  *SIGHUP*|*SIGINT*|*SIGKILL*|*SIGTERM*)
    printf '%s\n' "FAIL: kill -l output contains SIG prefix" >&2
    exit 1
    ;;
esac

# Check names are uppercase (HUP not hup)
case "$_out" in
  *hup*|*int*|*kill*|*term*)
    printf '%s\n' "FAIL: kill -l output contains lowercase signal names" >&2
    exit 1
    ;;
esac

# Check output ends with newline (printf %s trims it, so compare)
_raw=$(kill -l 2>/dev/null; printf x)
_raw=${_raw%x}
case "$_raw" in
  *"
")  ;;
  *)
    printf '%s\n' "FAIL: kill -l output does not end with newline" >&2
    exit 1
    ;;
esac

exit 0
