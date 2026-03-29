# Test: vi Line Editing
# Target: tests/matrix/tests/vi_editing.sh
#
# Tests POSIX vi-mode line editing using the expect_pty scriptable PTY driver.
# Each test sends raw keystrokes and verifies the shell's output.

. "$MATRIX_DIR/lib.sh"

# ==============================================================================
# Entering vi Mode and Basic Insert
# ==============================================================================
# REQUIREMENT: SHALL-Command-Line-Editing-032:
# The command set -o vi shall enable vi-mode editing and place sh into vi
# insert mode.
# REQUIREMENT: SHALL-Command-Line-Editing-033:
# This command also shall disable any other editing mode.
# REQUIREMENT: SHALL-Command-Line-Editing-vi-mode-034:
# In vi editing mode, there shall be a distinguished line, the edit line.
# REQUIREMENT: SHALL-Command-Line-Editing-vi-mode-035:
# When in insert mode, an entered character shall be inserted into the command
# line.
# REQUIREMENT: SHALL-Command-Line-Editing-vi-mode-036:
# Upon entering sh and after termination of the previous command, sh shall be
# in insert mode.
# REQUIREMENT: SHALL-vi-Line-Editing-Insert-Mode-042:
# While in insert mode, any character typed shall be inserted in the current
# command line.

assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -o vi"
expect "$ "
send "echo vi_mode_works"
expect "vi_mode_works"
expect "$ "
sendeof
wait'

# ==============================================================================
# ESC switches to command mode
# ==============================================================================
# REQUIREMENT: SHALL-Command-Line-Editing-vi-mode-037:
# Typing an escape character shall switch sh into command mode.
# REQUIREMENT: SHALL-Command-Line-Editing-vi-mode-038:
# In command mode, an entered character shall either invoke a defined operation,
# be used as part of a multi-character operation, or be treated as an error.
# REQUIREMENT: SHALL-SH-1023-DUP665:
# The following commands shall be recognized in command mode: <newline>
# Execute the current command line.

# Type "echo hellox", ESC to command mode, "x" deletes last char, Enter executes
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -o vi"
expect "$ "
sendraw 65 63 68 6f 20 68 65 6c 6c 6f 78
sendraw 1b
sleep 100
sendraw 78
sleep 100
sendraw 0a
expect "hello"
not_expect "hellox"
expect "$ "
sendeof
wait'

# ==============================================================================
# Replace character (r)
# ==============================================================================
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-096:
# Replace the current character with the character c.

# Type "echo hellx", ESC, r, o -> "hello"
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -o vi"
expect "$ "
sendraw 65 63 68 6f 20 68 65 6c 6c 78
sendraw 1b
sleep 100
sendraw 72 6f
sleep 100
sendraw 0a
expect "hello"
expect "$ "
sendeof
wait'

# ==============================================================================
# Case inversion (~)
# ==============================================================================
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-071:
# The current cursor position then shall be advanced by one character.
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-072:
# If the cursor was positioned on the last character of the line, the case
# conversion shall occur, but the cursor shall not advance.
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-073:
# If the ~ command is preceded by a count, that number of characters shall
# be converted.

# Type "echo aBC", ESC, 0 (start of line), w (skip to "aBC"), ~~~ -> "Abc"
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -o vi"
expect "$ "
sendraw 65 63 68 6f 20 61 42 43
sendraw 1b
sleep 100
sendraw 30 77
sleep 50
sendraw 7e 7e 7e
sleep 100
sendraw 0a
expect "Abc"
expect "$ "
sendeof
wait'

# ==============================================================================
# Cursor movement: h (left) and l (right)
# ==============================================================================
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-079:
# If the cursor was positioned on the first character of the line, the
# terminal shall be alerted and the cursor shall not be moved.
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-081:
# If the cursor was positioned on the last character of the line, the
# terminal shall be alerted and the cursor shall not be advanced.

# Type "echo abcd", ESC, hh (move left 2), ra (replace with 'a'), Enter
# "abcd" -> cursor on 'd', hh moves to 'b', ra replaces 'b' with 'a' -> "aacd"
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -o vi"
expect "$ "
sendraw 65 63 68 6f 20 61 62 63 64
sendraw 1b
sleep 100
sendraw 68 68 72 61
sleep 100
sendraw 0a
expect "aacd"
expect "$ "
sendeof
wait'

# ==============================================================================
# Word movement: w (next word) and b (back word)
# ==============================================================================
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-082:
# If the count is larger than the number of words after the cursor, this
# shall not be considered an error; the cursor shall advance to the last
# character on the line.
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-090:
# If the count is larger than the number of words preceding the cursor,
# this shall not be considered an error.

# Type "echo one two", ESC, b (back to "two"), rX (replace 't' with 'X') -> "one Xwo"
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -o vi"
expect "$ "
sendraw 65 63 68 6f 20 6f 6e 65 20 74 77 6f
sendraw 1b
sleep 100
sendraw 62 72 58
sleep 100
sendraw 0a
expect "one Xwo"
expect "$ "
sendeof
wait'

# ==============================================================================
# Delete character (x) and (X)
# ==============================================================================
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-114:
# If the cursor was positioned on the last character of the line, the
# character shall be deleted and the cursor position shall be moved back.
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-117:
# If the cursor was positioned on the first character of the line, the
# terminal shall be alerted, and the X command shall have no effect.
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-121:
# The deleted characters shall be placed in the save buffer.

# Type "echo abc", ESC, x (delete 'c') -> "ab"
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -o vi"
expect "$ "
sendraw 65 63 68 6f 20 61 62 63
sendraw 1b
sleep 100
sendraw 78
sleep 100
sendraw 0a
expect "ab"
expect "$ "
sendeof
wait'

# ==============================================================================
# Count prefix
# ==============================================================================
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-047:
# Decimal digits not beginning with 0 that precede a command letter shall
# be remembered.
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-051:
# Any command that is preceded by count shall take a count.
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-052:
# Unless otherwise noted, this count shall cause the specified operation
# to repeat by the number of times specified by the count.

# Type "echo abcde", ESC, 3x (delete 3 chars from right) -> "ab"
# Cursor is on 'e' after ESC, but x deletes under cursor. With 3x: delete 'e','d','c' -> "ab"
# Actually cursor lands on 'e' (last typed), ESC leaves it on 'e', 3x deletes 'e','d','c'? 
# In vi, ESC moves back one, so cursor is on 'd'. 3x deletes d,e -> wait, let me be precise.
# After typing "abcde", ESC puts cursor on 'e'. 3x deletes 'e' then 'd' then 'c' -> "ab"
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -o vi"
expect "$ "
sendraw 65 63 68 6f 20 61 62 63 64 65
sendraw 1b
sleep 100
sendraw 33 78
sleep 100
sendraw 0a
expect "ab"
expect "$ "
sendeof
wait'

# ==============================================================================
# Append (a) and Insert (i)
# ==============================================================================
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-105:
# Characters that are entered shall be inserted before the next character.
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-106:
# Characters that are entered shall be inserted before the current character.

# Type "echo ac", ESC, h (move to 'a'), a (append after 'a'), type 'b', ESC, Enter -> "abc"
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -o vi"
expect "$ "
sendraw 65 63 68 6f 20 61 63
sendraw 1b
sleep 100
sendraw 68 61 62
sendraw 1b
sleep 100
sendraw 0a
expect "abc"
expect "$ "
sendeof
wait'

# ==============================================================================
# Delete with motion (d + motion)
# ==============================================================================
# REQUIREMENT: SHALL-SH-1036:
# If count is specified, it shall be applied to the motion command.
# REQUIREMENT: SHALL-SH-1037:
# A count shall be ignored for the following motion commands: 0 ^ $ c
# REQUIREMENT: SHALL-SH-1038:
# If the motion command would move toward the beginning of the command line,
# the character under the current cursor position shall not be deleted.
# REQUIREMENT: SHALL-SH-1039:
# If the motion command is d, the entire current command line shall be cleared.
# REQUIREMENT: SHALL-SH-1040:
# If the count is larger than the number of characters between the current
# cursor position and the end of the command line, all remaining characters
# shall be deleted.
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-108:
# If the motion command would move the current cursor position toward the
# beginning of the command line, the character under the cursor and all
# characters to the end of the motion shall be deleted.
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-111:
# If the motion command is invalid, the terminal shall be alerted, the
# cursor shall not be moved, and no text shall be deleted.

# Type "echo hello world", ESC, dw (delete word from cursor) 
# Cursor on 'd' after ESC, dw would delete " world"? Actually after ESC, cursor is on 'd'.
# Let's use db: Type "echo hello world", ESC, b moves to 'w', dw deletes "world" -> "echo hello "
# Better: type "echo ab cd", ESC, puts cursor on 'd', db deletes back-word "cd" leaving "echo ab "
# Type "echo ab cd", ESC -> cursor on 'd', b -> cursor on 'c', dw -> deletes "cd" -> "echo ab "
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -o vi"
expect "$ "
sendraw 65 63 68 6f 20 61 62 20 63 64
sendraw 1b
sleep 100
sendraw 62
sleep 50
sendraw 64 77
sleep 100
sendraw 0a
expect "ab"
expect "$ "
sendeof
wait'

# ==============================================================================
# Change with motion (c + motion)  
# ==============================================================================
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-107:
# If the motion command is the character 'c', the current command line shall
# be cleared and insert mode shall be entered.
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-112:
# After this command, the current cursor position shall be on the last
# character that was changed.

# Type "echo old", ESC, cc (change whole line), type "echo new", Enter -> "new"
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -o vi"
expect "$ "
sendraw 65 63 68 6f 20 6f 6c 64
sendraw 1b
sleep 100
sendraw 63 63
sleep 50
sendraw 65 63 68 6f 20 6e 65 77
sendraw 0a
expect "new"
expect "$ "
sendeof
wait'

# ==============================================================================
# History navigation: k (previous) and j (next)
# ==============================================================================
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-129:
# If count is not specified, it shall default to 1.
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-130:
# The cursor shall be positioned on the first character of the new command.

# Run "echo hist1", then on new prompt, ESC, k to recall previous command, Enter
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -o vi"
expect "$ "
send "echo hist1"
expect "hist1"
expect "$ "
sendraw 1b
sleep 100
sendraw 6b
sleep 100
sendraw 0a
expect "hist1"
expect "$ "
sendeof
wait'

# ==============================================================================
# Go to beginning (0) and end ($)
# ==============================================================================
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-093:
# The first character position shall be numbered 1.

# Type "echo XY", ESC, 0 (beginning), r (replace) Z -> "Zcho XY"? 
# Actually 0 goes to first char of the line which is 'e' in "echo XY".
# So: type "echo XY", ESC, 0, rZ -> "Zcho XY"
# But that changes the command. Let's test $ instead:
# Type "echo ab", ESC, 0 (to 'e'), $ (to 'b'), rZ -> "echo aZ"
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -o vi"
expect "$ "
sendraw 65 63 68 6f 20 61 62
sendraw 1b
sleep 100
sendraw 30
sleep 50
sendraw 24
sleep 50
sendraw 72 5a
sleep 100
sendraw 0a
expect "aZ"
expect "$ "
sendeof
wait'

# ==============================================================================
# Undo (u)
# ==============================================================================
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-127:
# This operation shall not undo the copy of any command line to the edit line.

# Type "echo abc", ESC, x (delete 'c'), u (undo) -> restores 'c', Enter -> "abc"
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -o vi"
expect "$ "
sendraw 65 63 68 6f 20 61 62 63
sendraw 1b
sleep 100
sendraw 78
sleep 50
sendraw 75
sleep 100
sendraw 0a
expect "abc"
expect "$ "
sendeof
wait'

# ==============================================================================
# Dot repeat (.)
# ==============================================================================
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-075:
# If the previous command was preceded by a count, and no count is given on
# the '.' command, the count from the previous command shall be included.
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-076:
# If the '.' command is preceded by a count, this shall override any count
# argument to the previous command.

# Type "echo abcd", ESC, x (delete 'd'), . (repeat -> delete 'c') -> "ab"
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -o vi"
expect "$ "
sendraw 65 63 68 6f 20 61 62 63 64
sendraw 1b
sleep 100
sendraw 78
sleep 50
sendraw 2e
sleep 100
sendraw 0a
expect "ab"
expect "$ "
sendeof
wait'

# ==============================================================================
# Put (p) after delete
# ==============================================================================
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-123:
# The current cursor position shall be advanced to the last character put
# from the save buffer.
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-124:
# A count shall indicate how many copies of the save buffer shall be put.
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-125:
# The current cursor position shall be moved to the last character put from
# the save buffer.

# Type "echo abc", ESC, x (delete 'c' into save buffer), h (move to 'a'),
# p (put after 'a') -> "acb" 
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -o vi"
expect "$ "
sendraw 65 63 68 6f 20 61 62 63
sendraw 1b
sleep 100
sendraw 78
sleep 50
sendraw 68
sleep 50
sendraw 70
sleep 100
sendraw 0a
expect "acb"
expect "$ "
sendeof
wait'

# ==============================================================================
# SIGINT in command mode
# ==============================================================================
# REQUIREMENT: SHALL-Command-Line-Editing-vi-mode-040:
# If sh receives a SIGINT signal in command mode, it shall terminate command
# line editing on the current command line, reissue the prompt on the next
# line of the terminal.
# REQUIREMENT: SHALL-ASYNCHRONOUS-EVENTS-026:
# SIGINT signals received during command line editing shall be handled.
# REQUIREMENT: SHALL-vi-Line-Editing-Insert-Mode-045:
# If sh receives a SIGINT signal in insert mode, it shall terminate command
# line editing with the same effects.

# Type partial text, ESC, send Ctrl-C, verify we get a new prompt
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -o vi"
expect "$ "
sendraw 70 61 72 74 69 61 6c
sendraw 1b
sleep 100
sendraw 03
expect "$ "
send "echo after_sigint"
expect "after_sigint"
expect "$ "
sendeof
wait'

# ==============================================================================
# Insert mode: entering commands into history
# ==============================================================================
# REQUIREMENT: SHALL-vi-Line-Editing-Insert-Mode-043:
# If the current command line is not empty, this line shall be entered into
# the command history.
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-056:
# This line shall be entered into the command history; see fc.

# Type a command, then recall it from history on the next prompt
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -o vi"
expect "$ "
send "echo history_test_entry"
expect "history_test_entry"
expect "$ "
sendraw 1b
sleep 100
sendraw 6b
sleep 100
sendraw 0a
expect "history_test_entry"
expect "$ "
sendeof
wait'

# ==============================================================================
# Unrecognized command alerts terminal
# ==============================================================================
# REQUIREMENT: SHALL-Command-Line-Editing-vi-mode-039:
# A character that is not recognized as part of an editing command shall
# terminate any specific editing command and shall alert the terminal.

# Type "echo ok", ESC, Z (not a vi command) should alert but not modify line
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -o vi"
expect "$ "
sendraw 65 63 68 6f 20 6f 6b
sendraw 1b
sleep 100
sendraw 5a
sleep 100
sendraw 0a
expect "ok"
expect "$ "
sendeof
wait'

# ==============================================================================
# Insert mode: backspace erases from screen and buffer
# ==============================================================================
# REQUIREMENT: SHALL-vi-Line-Editing-Insert-Mode-044:
# In insert mode, characters shall be erased from both the screen and the
# buffer when backspacing.

# Type "echo abX", backspace (removes X), Enter -> "ab"
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -o vi"
expect "$ "
sendraw 65 63 68 6f 20 61 62 58 7f
sendraw 0a
expect "ab"
not_expect "abX"
expect "$ "
sendeof
wait'

# ==============================================================================
# EOF interpretation at beginning of line
# ==============================================================================
# REQUIREMENT: SHALL-vi-Line-Editing-Insert-Mode-046:
# This interpretation shall occur only at the beginning of an input line.
# (Ctrl-D as EOF only at the beginning of line)

# Type some text, Ctrl-D in the middle of a line should NOT exit
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -o vi"
expect "$ "
sendraw 65 63 68 6f 20 61 04 62
sendraw 0a
expect "$ "
sendeof
wait'

# ==============================================================================
# Edit line semantics: modify from history replaces edit line
# ==============================================================================
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-048:
# If the current line is not the edit line, any command that modifies the
# current line shall cause the content of the current line to replace the
# content of the edit line.
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-049:
# The modification requested shall then be performed to the edit line.
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-050:
# When the current line is the edit line, the modification shall be done
# directly to the edit line.

# Run "echo from_hist", then recall with k, modify with rx, execute
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -o vi"
expect "$ "
send "echo from_hist"
expect "from_hist"
expect "$ "
sendraw 1b
sleep 100
sendraw 6b
sleep 100
sendraw 24
sleep 50
sendraw 72 58
sleep 100
sendraw 0a
expect "from_hisX"
expect "$ "
sendeof
wait'

# ==============================================================================
# Count out of range alerts terminal
# ==============================================================================
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-053:
# A count that is out of range is considered an error condition and shall
# alert the terminal, but neither the cursor position, nor the command line,
# shall change.

# Type "echo ab", ESC, 9l (9 right — more than available) should alert but
# the line should remain unchanged; execute anyway
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -o vi"
expect "$ "
sendraw 65 63 68 6f 20 61 62
sendraw 1b
sleep 100
sendraw 39 6c
sleep 100
sendraw 0a
expect "ab"
expect "$ "
sendeof
wait'

# ==============================================================================
# Tilde count overflow
# ==============================================================================
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-074:
# If the count is larger than the number of characters after the cursor,
# this shall not be considered an error; the cursor shall advance to the
# last character on the line.

# Type "echo aB", ESC, 0w (to 'a'), 9~ (toggle 9 — only 2 chars) -> "Ab"
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -o vi"
expect "$ "
sendraw 65 63 68 6f 20 61 42
sendraw 1b
sleep 100
sendraw 30 77
sleep 50
sendraw 39 7e
sleep 100
sendraw 0a
expect "Ab"
expect "$ "
sendeof
wait'

# ==============================================================================
# Dot repeat count propagation
# ==============================================================================
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-077:
# The count specified in the '.' command shall become the count for
# subsequent '.' commands issued without a count.

# Type "echo abcdef", ESC, 2x (delete 2: 'e','f'->gone), . (repeat 2x: 'c','d'->gone) -> "ab"
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -o vi"
expect "$ "
sendraw 65 63 68 6f 20 61 62 63 64 65 66
sendraw 1b
sleep 100
sendraw 32 78
sleep 100
sendraw 2e
sleep 100
sendraw 0a
expect "ab"
expect "$ "
sendeof
wait'

# ==============================================================================
# h count overflow
# ==============================================================================
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-080:
# If the count is larger than the number of characters before the cursor,
# this shall not be considered an error; the cursor shall move to the first
# character on the line.

# Type "echo ab", ESC, 0w (to 'a'), l (to 'b'), 99h (overflow back), rZ -> "Zcho ab"
# actually 0 goes to 'e', w goes to 'a', l to 'b', 99h to 'e', rZ->"Zcho ab"
# With TERM=xterm, readline displays "(arg: N)" noise for digit prefixes,
# so we match just the distinguishing replacement to confirm cursor position.
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -o vi"
expect "$ "
sendraw 65 63 68 6f 20 61 62
sendraw 1b
sleep 100
sendraw 30 77 6c
sleep 50
sendraw 39 39 68
sleep 50
sendraw 72 5a
sleep 100
sendraw 0a
expect "Zcho"
expect "$ "
sendeof
wait'

# ==============================================================================
# Bigword forward (W) and backward (B) and end (E)
# ==============================================================================
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-084:
# If the count is larger than the number of bigwords after the cursor,
# this shall not be considered an error; the cursor shall advance to the
# last character on the line.
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-092:
# If the count is larger than the number of bigwords preceding the cursor,
# this shall not be considered an error; the cursor shall return to the
# first character on the line.

# Type "echo a.b c.d", ESC, 0 (start), W (bigword forward to 'c'), rZ -> "echo a.b Z.d"
# 0 -> 'e', W -> 'a', W -> 'c', rZ -> 'Z'
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -o vi"
expect "$ "
sendraw 65 63 68 6f 20 61 2e 62 20 63 2e 64
sendraw 1b
sleep 100
sendraw 30
sleep 50
sendraw 57 57
sleep 50
sendraw 72 5a
sleep 100
sendraw 0a
expect "a.b Z.d"
expect "$ "
sendeof
wait'

# B: bigword backward
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -o vi"
expect "$ "
sendraw 65 63 68 6f 20 61 2e 62 20 63 2e 64
sendraw 1b
sleep 100
sendraw 42
sleep 50
sendraw 72 5a
sleep 100
sendraw 0a
expect "a.b Z.d"
not_expect "c.d"
expect "$ "
sendeof
wait'

# ==============================================================================
# Pipe column movement (|)
# ==============================================================================
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-094:
# If the count is larger than the number of characters on the line,
# this shall not be considered an error; the cursor shall be placed
# on the last character on the line.

# Type "echo abcde", ESC, 3| (move to column 3), rZ -> "ecZo abcde"
# column 3 is 'h' in "echo abcde", rZ->"ecZo abcde"
# With TERM=xterm, readline displays "(arg: N)" noise for digit prefixes,
# so we match just the distinguishing replacement to confirm cursor position.
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -o vi"
expect "$ "
sendraw 65 63 68 6f 20 61 62 63 64 65
sendraw 1b
sleep 100
sendraw 33 7c
sleep 50
sendraw 72 5a
sleep 100
sendraw 0a
expect "ecZo"
expect "$ "
sendeof
wait'

# ==============================================================================
# Find character: f, F, t, T
# ==============================================================================
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-098:
# If the character 'c' does not occur in the line before the current cursor
# position, the terminal shall be alerted and the cursor shall not be moved.

# f: find forward — type "echo abcb", ESC, 0 (start), fb (find 'b'), rZ -> "echo Zbcb"
# Actually: 0->'e', fb finds first 'b' after cursor which is at index 5 ('b')
# "echo abcb", cursor at 'e', fb->'b'(index 5), rZ->"echo Zbcb"
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -o vi"
expect "$ "
sendraw 65 63 68 6f 20 61 62 63 62
sendraw 1b
sleep 100
sendraw 30
sleep 50
sendraw 66 62
sleep 50
sendraw 72 5a
sleep 100
sendraw 0a
expect "Zcb"
expect "$ "
sendeof
wait'

# F: find backward
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -o vi"
expect "$ "
sendraw 65 63 68 6f 20 61 62 63
sendraw 1b
sleep 100
sendraw 46 61
sleep 50
sendraw 72 5a
sleep 100
sendraw 0a
expect "Zbc"
expect "$ "
sendeof
wait'

# t: find forward, stop before
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -o vi"
expect "$ "
sendraw 65 63 68 6f 20 61 62 63
sendraw 1b
sleep 100
sendraw 30
sleep 50
sendraw 74 63
sleep 50
sendraw 72 5a
sleep 100
sendraw 0a
expect "echo aZc"
expect "$ "
sendeof
wait'

# T: find backward, stop after
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -o vi"
expect "$ "
sendraw 65 63 68 6f 20 61 62 63
sendraw 1b
sleep 100
sendraw 54 61
sleep 50
sendraw 72 5a
sleep 100
sendraw 0a
expect "echo aZc"
expect "$ "
sendeof
wait'

# ==============================================================================
# Repeat find (;) and reverse repeat (,)
# ==============================================================================
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-103:
# Any number argument on that previous command shall be ignored.

# Type "echo abab", ESC, 0, fa (find 'a'), ; (repeat find) -> second 'a'
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -o vi"
expect "$ "
sendraw 65 63 68 6f 20 61 62 61 62
sendraw 1b
sleep 100
sendraw 30
sleep 50
sendraw 66 61
sleep 50
sendraw 3b
sleep 50
sendraw 72 5a
sleep 100
sendraw 0a
expect "abZb"
expect "$ "
sendeof
wait'

# , (reverse repeat): find forward then reverse
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -o vi"
expect "$ "
sendraw 65 63 68 6f 20 61 62 61 62
sendraw 1b
sleep 100
sendraw 30
sleep 50
sendraw 66 61
sleep 50
sendraw 3b
sleep 50
sendraw 2c
sleep 50
sendraw 72 5a
sleep 100
sendraw 0a
expect "Zbab"
expect "$ "
sendeof
wait'

# ==============================================================================
# Change motion toward end / count overflow
# ==============================================================================
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-109:
# If the motion command would move the current cursor position toward the
# end of the command line, the character under the current cursor position
# shall be deleted.
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-110:
# If the count is larger than the number of characters between the current
# cursor position and the end of the command line toward which the motion
# command would move the cursor, this shall not be considered an error;
# all of the remaining characters in the aforementioned range shall be
# deleted and insert mode shall be entered.

# Type "echo abcd", ESC, 0w (to 'a'), cw (change word), type "XY", ESC -> "echo XYcd"
# Actually cw changes from cursor to end of word: 0->'e', w->'a', cw deletes "abcd", type "XY"
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -o vi"
expect "$ "
sendraw 65 63 68 6f 20 61 62 63 64
sendraw 1b
sleep 100
sendraw 30 77
sleep 50
sendraw 63 77
sleep 50
sendraw 58 59
sendraw 1b
sleep 100
sendraw 0a
expect "XY"
expect "$ "
sendeof
wait'

# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-113:
# If the count is larger than the number of characters after the cursor,
# this shall not be considered an error; all of the remaining characters
# shall be changed.

# Type "echo ab", ESC, 0w (to 'a'), 9cw (change 9 words — overflow), type "Z", ESC
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -o vi"
expect "$ "
sendraw 65 63 68 6f 20 61 62
sendraw 1b
sleep 100
sendraw 30 77
sleep 50
sendraw 39 63 77
sleep 50
sendraw 5a
sendraw 1b
sleep 100
sendraw 0a
expect "Z"
not_expect "ab"
expect "$ "
sendeof
wait'

# ==============================================================================
# Delete x count overflow
# ==============================================================================
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-115:
# If the count is larger than the number of characters after the cursor,
# this shall not be considered an error; all the characters from the cursor
# to the end of the line shall be deleted.

# Type "echo ab", ESC, 0w (to 'a'), 9x (delete 9 — only 2 avail), Enter -> ""
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -o vi"
expect "$ "
sendraw 65 63 68 6f 20 61 62
sendraw 1b
sleep 100
sendraw 30 77
sleep 50
sendraw 39 78
sleep 100
sendraw 0a
expect "$ "
sendeof
wait'

# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-116:
# The character under the current cursor position shall not change.
# (X deletes before cursor; char under cursor stays)

# Type "echo abc", ESC, $ (to 'c'), X (delete 'b'), verify 'c' stays -> "ac"
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -o vi"
expect "$ "
sendraw 65 63 68 6f 20 61 62 63
sendraw 1b
sleep 100
sendraw 58
sleep 100
sendraw 0a
expect "ac"
expect "$ "
sendeof
wait'

# ==============================================================================
# X edge cases
# ==============================================================================
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-119:
# If the line contained no characters, the terminal shall be alerted and the
# cursor shall not be moved.
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-118:
# If the line contained a single character, the X command shall have no effect.

# Type "echo a", ESC, 0w (to 'a'), X (no effect — cursor on first char of 'a'), Enter
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -o vi"
expect "$ "
sendraw 65 63 68 6f 20 61
sendraw 1b
sleep 100
sendraw 30 77
sleep 50
sendraw 58
sleep 100
sendraw 0a
expect "a"
expect "$ "
sendeof
wait'

# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-120:
# If the count is larger than the number of characters before the cursor,
# this shall not be considered an error; all the characters from before
# the cursor to the beginning of the line shall be deleted.

# Type "echo abcd", ESC, 99X (delete everything before cursor) -> leaves just last char
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -o vi"
expect "$ "
sendraw 65 63 68 6f 20 61 62 63 64
sendraw 1b
sleep 100
sendraw 39 39 58
sleep 100
sendraw 0a
expect "$ "
sendeof
wait'

# ==============================================================================
# Yank (y + motion): cursor position unchanged
# ==============================================================================
# REQUIREMENT: SHALL-SH-1041:
# A number count shall be applied to the motion command.
# REQUIREMENT: SHALL-SH-1042:
# If the motion command would move toward the beginning of the command line,
# the character under the current cursor position shall not be included in
# the set of yanked characters.
# REQUIREMENT: SHALL-SH-1043:
# If the motion command is y, the entire current command line shall be yanked
# into the save buffer.
# REQUIREMENT: SHALL-SH-1044:
# The current cursor position shall be unchanged.
# REQUIREMENT: SHALL-SH-1045:
# If the count is larger than the number of characters between the cursor
# and the end of the line, all remaining characters shall be yanked.
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-122:
# The current character position shall be unchanged.

# Type "echo abc", ESC, 0 (to 'e'), yw (yank word), cursor stays at 'e',
# rZ -> "Zcho abc"
# With TERM=xterm, readline may display "(arg: N)" noise for some key
# sequences, so we match just the distinguishing replacement.
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -o vi"
expect "$ "
sendraw 65 63 68 6f 20 61 62 63
sendraw 1b
sleep 100
sendraw 30
sleep 50
sendraw 79 77
sleep 50
sendraw 72 5a
sleep 100
sendraw 0a
expect "Zcho"
expect "$ "
sendeof
wait'

# ==============================================================================
# History k/- past HISTSIZE boundary
# ==============================================================================
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-131:
# If a k or - command would retreat past the maximum number of commands in
# effect for this shell, the terminal shall be alerted, and the command
# shall have no effect.

# Run a single command, then try k twice (second should alert but not change)
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -o vi"
expect "$ "
send "echo only_cmd"
expect "only_cmd"
expect "$ "
sendraw 1b
sleep 100
sendraw 6b
sleep 100
sendraw 6b
sleep 100
sendraw 0a
expect "only_cmd"
expect "$ "
sendeof
wait'

# ==============================================================================
# History j/+ past edit line
# ==============================================================================
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-134:
# If a j or + command advances past the edit line, the current command line
# shall be restored to the edit line and the terminal shall be alerted.

# Run "echo hist_a", recall with k, then j should restore empty edit line
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -o vi"
expect "$ "
send "echo hist_a"
expect "hist_a"
expect "$ "
sendraw 1b
sleep 100
sendraw 6b
sleep 100
sendraw 6a
sleep 100
sendraw 0a
expect "$ "
sendeof
wait'

# ==============================================================================
# History G with nonexistent line number
# ==============================================================================
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-135:
# If command line number does not exist, the terminal shall be alerted
# and the command line shall not be changed.

# Type some text, ESC, 99999G (nonexistent line), line should not change
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -o vi"
expect "$ "
send "echo baseline"
expect "baseline"
expect "$ "
sendraw 1b
sleep 100
sendraw 39 39 39 39 39 47
sleep 100
sendraw 0a
expect "$ "
sendeof
wait'

# ==============================================================================
# HISTSIZE unset default
# ==============================================================================
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-136:
# If this variable is unset, an unspecified default greater than or equal
# to 128 shall be used.

# Unset HISTSIZE, run >1 command, recall with k — should work
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -o vi"
expect "$ "
send "unset HISTSIZE"
expect "$ "
send "echo histtest1"
expect "histtest1"
expect "$ "
sendraw 1b
sleep 100
sendraw 6b
sleep 100
sendraw 0a
expect "histtest1"
expect "$ "
sendeof
wait'

# ==============================================================================
# History search / and ?
# ==============================================================================
# REQUIREMENT: SHALL-SH-1046:
# Patterns use pattern matching notation, except that ^ shall have special
# meaning when it appears as the first character of pattern.
# REQUIREMENT: SHALL-SH-1047:
# The ^ is discarded and the characters after the ^ shall be matched only
# at the beginning of a line.
# REQUIREMENT: SHALL-SH-1049:
# If the pattern is not found, the current command line shall be unchanged
# and the terminal shall be alerted.
# REQUIREMENT: SHALL-SH-1050:
# If it is found in a previous line, the current command line shall be set
# to that line and the cursor set to the first character.
# REQUIREMENT: SHALL-SH-1051:
# If it is found in a following line, the current command line shall be set
# to that line and the cursor set to the first character.
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-137:
# If there is no previous non-empty pattern, the terminal shall be alerted
# and the current command line shall remain unchanged.
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-138:
# If pattern is empty, the last non-empty pattern provided to / or ?
# shall be used.
# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-140:
# If there is no previous / or ?, the terminal shall be alerted and the
# current command line shall remain unchanged.

# Run commands, then /pattern to search backward
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -o vi"
expect "$ "
send "echo searchme"
expect "searchme"
expect "$ "
send "echo other"
expect "other"
expect "$ "
sendraw 1b
sleep 100
sendraw 2f 73 65 61 72 63 68 0a
sleep 200
sendraw 0a
expect "searchme"
expect "$ "
sendeof
wait'

# REQUIREMENT: SHALL-vi-Line-Editing-Command-Mode-139:
# If no matching command line is found, the terminal shall be alerted
# and the current command line shall remain unchanged.

# Search for nonexistent pattern
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -o vi"
expect "$ "
send "echo first"
expect "first"
expect "$ "
sendraw 1b
sleep 100
sendraw 2f 7a 7a 7a 7a 7a 7a 0a
sleep 200
sendraw 0a
expect "$ "
sendeof
wait'

report
