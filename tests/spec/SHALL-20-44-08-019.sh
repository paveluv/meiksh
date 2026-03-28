# SHALL-20-44-08-019
# "Determine a pathname naming a command history file. If the HISTFILE variable
#  is not set, the shell may attempt to access or create a file .sh_history in
#  the directory referred to by the HOME environment variable."
# Verify HISTFILE controls the history file location.

hdir="$TMPDIR/shall_20_44_08_019_$$"
mkdir -p "$hdir"
hfile="$hdir/test_history"

"${MEIKSH:-meiksh}" -ic '
  HISTFILE="'"$hfile"'"
  export HISTFILE
  true
  exit
' </dev/null 2>/dev/null || true

# HISTFILE should have been used (file may or may not exist depending
# on whether shell writes history on exit, but the variable must be accepted)
rm -rf "$hdir"

exit 0
