# SHALL-20-44-11-001
# "The standard error shall be used only for diagnostic messages."
# Verify fc -l writes output to stdout, not stderr.

set -e
HISTFILE="$TMPDIR/hist_$$"
export HISTFILE
HISTSIZE=100
export HISTSIZE

${SHELL} -c '
  HISTFILE="'"$HISTFILE"'"
  HISTSIZE=100
  echo stderr_test
  # Capture stderr separately
  errout=$(fc -l -1 -1 2>&1 1>/dev/null)
  if [ -n "$errout" ]; then
    echo "FAIL: fc -l wrote non-diagnostic output to stderr: $errout" >&2
    exit 1
  fi
  exit 0
'

rm -f "$HISTFILE"
exit 0
