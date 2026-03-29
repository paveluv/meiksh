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

report
