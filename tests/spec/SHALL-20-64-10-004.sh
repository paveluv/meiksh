# SHALL-20-64-10-004
# "When both the -l option and exit_status operand are specified, the
#  symbolic name of the corresponding signal shall be written in the
#  following format:"
# Verify: kill -l <exit_status> outputs a single signal name followed
#  by newline.

# kill -l 9 -> "KILL\n"
_raw=$(kill -l 9 2>/dev/null; printf x)
_raw=${_raw%x}

# Must contain KILL
case "$_raw" in
  *KILL*) ;;
  *)
    printf '%s\n' "FAIL: kill -l 9 output '$_raw' does not contain KILL" >&2
    exit 1
    ;;
esac

# Must end with newline
case "$_raw" in
  *"
")  ;;
  *)
    printf '%s\n' "FAIL: kill -l 9 output not terminated by newline" >&2
    exit 1
    ;;
esac

# Must not contain SIG prefix
case "$_raw" in
  *SIGKILL*)
    printf '%s\n' "FAIL: kill -l 9 output contains SIG prefix" >&2
    exit 1
    ;;
esac

exit 0
