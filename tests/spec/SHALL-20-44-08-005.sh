# SHALL-20-44-08-005
# "Determine a pathname naming a command history file. If the HISTFILE variable
#  is not set, the shell may attempt to access or create a file .sh_history in
#  the directory referred to by the HOME environment variable. [...] As entries
#  are deleted from the history file, they shall be deleted oldest first."
# Verify HISTFILE controls history file path and history operates when set.

set -e
HISTFILE="$TMPDIR/hist_path_test_$$"
export HISTFILE
HISTSIZE=100
export HISTSIZE

# Remove any pre-existing file
rm -f "$HISTFILE"

${SHELL} -c '
  HISTFILE="'"$HISTFILE"'"
  HISTSIZE=100
  echo histfile_path_verify
  fc -l -1 -1 > /dev/null
  exit 0
'

if [ -f "$HISTFILE" ]; then
  rm -f "$HISTFILE"
  exit 0
else
  echo "FAIL: HISTFILE not created at specified path" >&2
  exit 1
fi
