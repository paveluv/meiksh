# Emacs Editing Mode

## Status

**Not implemented.** This document is a specification; no supporting code exists in meiksh yet. The `emacs` shell option is not accepted by `set -o`, no `src/interactive/emacs_editing.rs` is present, and the `bind` builtin is not registered. The four implementation stages (plumbing, core editor, inputrc parser, `bind` builtin) are scoped but not scheduled. User-visible behavior shall be verified by integration tests under [tests/integration/emacs_mode.rs](../../tests/integration/emacs_mode.rs) driven through the shared PTY harness in [tests/integration/interactive_common/](../../tests/integration/interactive_common/); pure-logic pieces (keymap, kill buffer, undo, inputrc parser) are covered by colocated `#[cfg(test)]` unit tests alongside their implementation.

## 1. Scope

This document is the authoritative specification of the emacs-style interactive line-editing mode provided by meiksh. It also specifies the `bind` builtin, which is the runtime interface used to inspect and reconfigure key bindings.

No external standard describes emacs-mode line editing. POSIX.1-2024 (Issue 8) specifies only `vi` mode in the `sh` utility; the GNU Readline library manual and the GNU Bash manual are product documentation, not specifications. The ksh93 and zsh manuals document their own independent implementations. Meiksh therefore owns its own spec for this feature, informed by those documents but accountable only to the text below.

### 1.1 Conformance Language

The key words "shall", "shall not", "should", "should not", "may", and "must" in this document are to be interpreted as described in RFC 2119. The text following a bulleted `shall` requirement constitutes a normative requirement that meiksh conformance tests verify.

### 1.2 De-Facto Reference

Where this document intentionally aligns with existing practice, the de-facto reference is:

- GNU Bash 5.2 with GNU Readline 8.x, `editing-mode emacs`.
- ksh93 emacs mode (`set -o emacs`) as documented in the ksh93 manual.
- zsh with `bindkey -e` as documented in `zshzle(1)`.

Meiksh aligns with bash readline where the three references diverge.

### 1.3 Non-Goals

This specification intentionally omits a number of features that exist in the reference shells. The omissions are listed normatively in Section 15 (Non-Goals). Appendix B describes what it would take to add each omitted feature later. The absence of a feature from this document shall not be interpreted as an oversight.

## 2. Activation and Lifecycle

### 2.1 Enabling and Disabling

- `set -o emacs` shall enable emacs editing mode.
- `set +o emacs` shall disable emacs editing mode.
- When emacs editing mode is disabled and no other editing mode is active, the shell shall read input lines using canonical (line-buffered) terminal input.
- The reportable options output produced by `set -o` shall include the line `emacs            on` or `emacs            off` reflecting the current state, using the same column formatting used by the other POSIX `set -o` options.

### 2.2 Mutual Exclusion With Vi Mode

- If `set -o emacs` is executed while vi mode is enabled, vi mode shall be disabled as a side effect.
- If `set -o vi` is executed while emacs mode is enabled, emacs mode shall be disabled as a side effect.
- At most one of `emacs` and `vi` shall be enabled at any given time. This is a meiksh deviation from bash, where `set -o` treats the two options as loosely coupled; in meiksh the coupling is normative.

### 2.3 Non-Interactive Shells

- A non-interactive shell shall ignore the `emacs` option. `set -o emacs` in a non-interactive shell shall update the reportable state but shall have no effect on input reading.
- A shell that becomes non-interactive mid-execution shall not re-enable canonical input mid-line; the current line completes under whatever mode was active when input began.

### 2.4 No Terminal Available

- When `emacs` is enabled but standard input is not a terminal, the shell shall silently fall back to canonical line-buffered input without attempting to put the descriptor into raw mode and without printing a diagnostic.
- The fallback shall match the current behavior of the vi-mode entry point.

### 2.5 Default State

- Meiksh has no default editing mode. On shell startup, neither `emacs` nor `vi` is enabled unless explicitly requested. This is a meiksh deviation from bash (which defaults to emacs) and from ksh (which infers the mode from `$VISUAL` / `$EDITOR`).

## 3. Terminal Preconditions

On entry to the emacs editor for each input line, meiksh shall configure the controlling terminal as follows.

### 3.1 Raw Mode

- The current terminal attributes shall be saved.
- The `ICANON`, `ECHO`, and `ISIG` local-mode flags shall be cleared.
- `c_cc[VMIN]` shall be set to 1 and `c_cc[VTIME]` to 0.
- Other flags shall be left unchanged.
- The saved attributes shall be restored before returning an input line, on signal interrupts handled by the editor, and on all error paths that exit the editor.

### 3.2 Escape Sequences

- The editor shall not emit terminal keypad application-mode sequences. Input parsing shall recognize both the normal-mode and application-mode ANSI sequences for arrow keys, `Home`, `End`, `Insert`, `Delete`, `Page Up`, `Page Down`, and the F-key range F1-F12.
- The editor shall redraw the current line using the ANSI sequence `\r\x1b[K` followed by the prompt and the current buffer contents.

## 4. Key Notation

Throughout this document and in `bind` output:

- `C-x` denotes the byte value produced by holding Control and pressing x. For alphabetic keys this is the value `x & 0x1F`.
- `M-x` denotes the two-byte sequence `ESC` followed by the byte for x. Meiksh recognizes meta only in its ESC-prefix form.
- `RET` denotes the ASCII carriage return (0x0D); meiksh shall accept both `RET` and `LF` (0x0A) as the line terminator.
- `DEL` denotes the ASCII value 0x7F; `BS` denotes 0x08. Either shall be accepted as backward-delete-char unless explicitly overridden by the user's erase character retrieved from the terminal attributes.
- `TAB` denotes the ASCII value 0x09.
- `NUL` denotes 0x00.

## 5. Bindings

This section enumerates every key sequence that meiksh shall bind by default and the exact behavior of each bindable function. Bindings not listed here are not bound by default; however, every function named below shall be addressable by the `bind` builtin even if unbound by default (for example, `history-search-backward`).

### 5.1 Basic Movement

| Key | Function | Behavior |
|---|---|---|
| `C-a` | `beginning-of-line` | The cursor shall move to the first byte of the input buffer. |
| `C-e` | `end-of-line` | The cursor shall move past the last byte of the input buffer. |
| `C-f` | `forward-char` | The cursor shall advance by one character (one grapheme; multibyte-aware, see Section 12). At end of buffer, the bell shall ring. |
| `C-b` | `backward-char` | The cursor shall move back by one character. At beginning of buffer, the bell shall ring. |
| `M-f` | `forward-word` | The cursor shall advance past the end of the next word. A word is defined as a maximal run of characters in the `alnum` locale class plus `_`. |
| `M-b` | `backward-word` | The cursor shall move to the beginning of the current or previous word. |
| `C-l` | `clear-screen` | The terminal screen shall be cleared and the prompt and current line redisplayed at the top. |

### 5.2 Cursor Keys

- `Up` / `\e[A` / `\eOA` shall be bound to `previous-history`.
- `Down` / `\e[B` / `\eOB` shall be bound to `next-history`.
- `Right` / `\e[C` / `\eOC` shall be bound to `forward-char`.
- `Left` / `\e[D` / `\eOD` shall be bound to `backward-char`.
- `Home` / `\e[H` / `\eOH` / `\e[1~` shall be bound to `beginning-of-line`.
- `End` / `\e[F` / `\eOF` / `\e[4~` shall be bound to `end-of-line`.
- `Ctrl+Right` / `\e[1;5C` shall be bound to `forward-word`.
- `Ctrl+Left` / `\e[1;5D` shall be bound to `backward-word`.
- `Page Up` / `\e[5~` shall be bound to `beginning-of-history`.
- `Page Down` / `\e[6~` shall be bound to `end-of-history`.
- `Delete` / `\e[3~` shall be bound to `delete-char`.

The functions `beginning-of-history`, `end-of-history`, `previous-history`, and `next-history` are bindable but have no default key sequence other than the cursor keys listed above.

### 5.3 History

| Key | Function | Behavior |
|---|---|---|
| `C-p` | `previous-history` | Replace the current buffer with the previous entry in the history list. If already at the oldest entry, ring the bell. |
| `C-n` | `next-history` | Replace the current buffer with the next entry. If the current buffer is not from history, the bell shall ring. If the current entry is the newest, restore the most recent user-edited buffer. |
| `C-r` | `reverse-search-history` | Enter incremental reverse search (see Section 7). |
| `C-s` | `forward-search-history` | Enter incremental forward search. If the terminal has XON/XOFF flow control enabled that would intercept `C-s`, meiksh shall nonetheless attempt to receive the byte; absence of delivery is a terminal limitation, not a spec failure. |
| `M-.` | `yank-last-arg` | Insert the last word of the previous history entry at the cursor. Repeated invocations shall walk backwards through history, each replacing the previously inserted word. The first non-`yank-last-arg` command shall terminate the walk. |
| `M-_` | `yank-last-arg` | Synonym for `M-.`. |

The following history functions shall be bindable but unbound by default. They exist so that users may bind them in `~/.inputrc`, typically to the arrow keys:

- `history-search-backward`: Search backwards for a history entry that begins with the text between the start of the buffer and the current cursor position. The cursor shall remain at its current column after replacement.
- `history-search-forward`: Search forwards with the same prefix-match semantics.

### 5.4 Deletion

| Key | Function | Behavior |
|---|---|---|
| `DEL` / `BS` | `backward-delete-char` | Delete the character before the cursor. At beginning of buffer, ring the bell. |
| `C-d` | `delete-char` | If the buffer is empty, the editor shall return end-of-file. Otherwise delete the character under the cursor. At end of buffer when buffer is non-empty, ring the bell. |
| `C-k` | `kill-line` | Delete from the cursor to end-of-buffer and place the deleted text into the kill buffer, following the kill-buffer semantics of Section 6. |
| `C-u` | `unix-line-discard` | Delete from the cursor backwards to beginning-of-buffer, placing the deleted text into the kill buffer. |
| `C-w` | `unix-word-rubout` | Delete from the cursor backwards to the previous whitespace (SPACE, TAB, NEWLINE), placing the deleted text into the kill buffer. Word boundaries for `C-w` shall be whitespace only, matching POSIX word semantics, not the `alnum+_` definition used by `M-f` / `M-b`. |
| `M-d` | `kill-word` | Delete from the cursor forwards to the end of the current word (using the `alnum+_` definition), placing the deleted text into the kill buffer. |
| `M-DEL` / `M-BS` | `backward-kill-word` | Delete from the cursor backwards to the beginning of the previous word (using the `alnum+_` definition), placing the deleted text into the kill buffer. |

### 5.5 Yank

| Key | Function | Behavior |
|---|---|---|
| `C-y` | `yank` | Insert the contents of the kill buffer at the cursor. The cursor shall be left after the inserted text. |

There is no `yank-pop`. The kill model is a single buffer (see Section 6).

### 5.6 Text Editing

| Key | Function | Behavior |
|---|---|---|
| `C-t` | `transpose-chars` | If the cursor is not at beginning-of-buffer and the buffer contains at least two characters, exchange the character before the cursor with the character at the cursor, then advance the cursor past both. If at end-of-buffer, exchange the two characters preceding the cursor. |
| `M-t` | `transpose-words` | Exchange the word at (or immediately before) the cursor with the preceding word. The cursor shall be positioned at the end of the second (now-later) word after the exchange. |
| `M-u` | `upcase-word` | Convert the current or next word to upper case; the cursor shall be left at end of word. |
| `M-l` | `downcase-word` | Convert the current or next word to lower case. |
| `M-c` | `capitalize-word` | Convert the first letter of the current or next word to upper case and the remainder to lower case. |
| `C-q` | `quoted-insert` | Read the next byte verbatim (bypassing keymap dispatch) and insert it at the cursor. |
| `C-v` | `quoted-insert` | Synonym for `C-q`. |

All printable bytes (0x20 through 0x7E) not otherwise bound, plus all bytes with the high bit set (forming UTF-8 continuation sequences), shall be bound to `self-insert`, which inserts the byte sequence at the cursor.

### 5.7 Completion

| Key | Function | Behavior |
|---|---|---|
| `TAB` | `complete` | Attempt completion on the word before the cursor (see Section 5.8). If no completion is possible, the bell shall ring. If exactly one completion is possible, it shall replace the partial word. If multiple completions are possible, the longest common prefix shall replace the partial word; if no additional characters can be added, the second consecutive `TAB` shall list the possible completions. |

### 5.8 Completion Algorithm

- The word before the cursor shall be extracted as the maximal trailing run of characters not equal to any of `SPACE`, `TAB`, `NEWLINE`, `>`, `<`, `|`, `;`, `(`, `)`, `&`, `` ` ``, or `"`.
- If the cursor is on the first word of the command line (i.e. no unquoted space precedes it in the buffer), meiksh shall attempt, in order:
  1. Command completion: match against shell builtins, aliases, functions, hashed command names, and `$PATH` executable names.
  2. Filename completion, if the partial word contains a `/`.
- Otherwise, meiksh shall attempt:
  1. Variable completion if the partial word begins with `$`.
  2. Tilde completion if the partial word begins with `~`.
  3. Filename completion using the pathname-expansion logic from `src/expand/glob.rs`.
- The terminating character of a single filename completion shall be a trailing `/` if the matched path is a directory (subject to `mark-symlinked-directories` for symlinks), or a trailing SPACE otherwise.
- When listing multiple completions on a second `TAB`, the candidates shall be printed one per column across the terminal width, sorted lexicographically; if the count of candidates exceeds `completion-query-items` (default 100) the shell shall not apply pagination but shall print the full list.
- When `show-all-if-ambiguous` is on, a single `TAB` on a partial word with multiple completions shall list them immediately rather than requiring a second `TAB`.
- When `completion-ignore-case` is on, case differences between the partial word and the candidate shall not prevent a match.

### 5.9 Miscellaneous

| Key | Function | Behavior |
|---|---|---|
| `RET` / `LF` | `accept-line` | Return the current buffer as the input line. |
| `C-_` | `undo` | Undo the most recent editing group (see Section 9). |
| `C-g` | `abort` | Abort the current composite action (incremental search, `quoted-insert`). If nothing composite is in progress, ring the bell. |
| `C-c` | `send-sigint` | Raise SIGINT for the shell process. The editor shall abort the current line, emit a newline, restore the terminal, and the shell shall redisplay the next prompt. |
| `C-x C-e` | `edit-and-execute-command` | Save the current buffer to a temporary file, invoke `$VISUAL` (or `$EDITOR`, or `vi`) on the file, and after the editor exits return the file contents as the input line. This shall reuse the implementation already used by vi mode. |

### 5.10 Bracketed Paste

When `enable-bracketed-paste` is on (the default, Section 3.2 of [inputrc.md](inputrc.md)):

- The shell shall emit the sequence `\e[?2004h` when entering the editor for each input line, and `\e[?2004l` when leaving.
- Between `\e[200~` and `\e[201~` the editor shall insert all bytes literally into the buffer, bypassing keymap dispatch.
- The pasted run shall be a single undo group (Section 9).
- A pasted run shall not alter the "consecutive kill" tracking used by the kill buffer (Section 6).

## 6. Kill-Buffer Semantics

- Meiksh shall maintain a single kill buffer per interactive shell session. The buffer shall be initialized empty.
- The following functions shall be considered kill commands: `kill-line`, `unix-line-discard`, `unix-word-rubout`, `kill-word`, `backward-kill-word`.
- If the previously dispatched command was a kill command, the current kill command shall append its deleted text to the kill buffer. `backward-kill-word` and `unix-word-rubout` shall prepend rather than append.
- If the previously dispatched command was not a kill command, the current kill command shall replace the kill buffer.
- `yank` shall insert the current contents of the kill buffer. It shall not modify the buffer.
- This specification provides a single buffer only; there is no ring, no `M-y`, and no rotation. See Appendix B, Package 1 for the work to add a ring later.

## 7. Incremental Search

Activated by `reverse-search-history` (`C-r`) or `forward-search-history` (`C-s`).

### 7.1 Mini-Buffer

- On entry, meiksh shall replace the normal prompt with `(reverse-i-search)\`\`: ` for reverse search or `(i-search)\`\`: ` for forward search.
- Each printable byte input shall be appended to the search pattern; the display shall update to show the pattern between the backticks and the current matching line after the colon.
- `DEL` / `BS` shall remove the most recent character from the pattern; the search shall re-execute against the abbreviated pattern from the same starting point as before the character was added.
- Repeating the activation key (`C-r` during reverse, `C-s` during forward) shall advance to the next matching history entry in the same direction.

### 7.2 Exiting

- `C-g` shall abort the search: the buffer and cursor shall be restored to their pre-search values and the normal prompt redisplayed.
- `RET` shall accept: the editor shall exit search with the current matching line as the buffer, cursor positioned at the end, and immediately call `accept-line`.
- Any other control character or escape sequence shall accept the current match and re-execute the received key or sequence against the main keymap. (This matches bash behavior and allows flows such as "`C-r` partial `C-a`" to leave the search positioned at the start of the matched line.)

### 7.3 Failure

- When no history entry matches the current pattern, the display shall prefix the mini-buffer prompt with `failing ` and the bell shall ring.
- Further `C-r` / `C-s` while failing shall keep the bell and mini-buffer in the failing state without moving the position.

## 8. Bracketed Paste Detail

See Section 5.10. No additional requirements.

## 9. Undo

- Meiksh shall maintain a per-line undo stack containing editing groups.
- An editing group is formed by one of:
  1. A maximal run of `self-insert` invocations with no intervening non-`self-insert` command. The run shall be committed as a single group on the first non-`self-insert` command or on `accept-line`.
  2. A single kill command.
  3. A single `yank`.
  4. A single `transpose-chars`, `transpose-words`, `upcase-word`, `downcase-word`, or `capitalize-word`.
  5. A single bracketed paste (Section 5.10).
- `undo` shall reverse the topmost group and remove it from the stack. If the stack is empty, the bell shall ring.
- The undo stack shall be cleared after `accept-line`. Undo does not cross line boundaries.

## 10. Signal Handling

- SIGINT received while the editor is waiting for input shall abort the current line: the editor shall emit a newline, restore terminal attributes, clear the input buffer, and the shell main loop shall redisplay the next prompt.
- SIGTSTP received while the editor is active shall restore terminal attributes, re-send itself with the default disposition (suspending the shell), and on receipt of SIGCONT shall re-enter raw mode and redraw the current line.
- SIGWINCH received while the editor is active shall cause the editor to recompute displayed column width at the next keystroke. The current line shall not be redrawn immediately; the redraw shall occur with the next user input.
- SIGHUP shall flush history per the rules already implemented by `src/interactive/history.rs` before the shell terminates.

## 11. End-of-File

- `C-d` received when the input buffer is empty shall return end-of-file to the REPL, causing an interactive shell with `ignoreeof` unset to exit.
- `C-d` on a non-empty buffer shall delete the character under the cursor (`delete-char`), regardless of cursor position relative to end-of-buffer.

## 12. Multibyte and Locale Handling

- All cursor motion, display-column calculation, and word-boundary detection shall operate on locale-decoded characters rather than raw bytes.
- Column width shall be obtained via `sys::locale::char_width`.
- Word boundaries for `forward-word`, `backward-word`, `kill-word`, and `backward-kill-word` shall be defined by `sys::locale::classify_char` with the `alnum` class, plus the ASCII character `_`.
- Invalid UTF-8 sequences shall be treated as single-byte characters with column width 1 to ensure the editor never stalls on malformed input.
- `self-insert` of a multibyte character shall insert all bytes atomically; it shall not be possible to leave the buffer containing an incomplete multibyte sequence.

## 13. Interaction With Other Builtins

### 13.1 `set -o` Reporting

- `set -o` output shall include a line for `emacs` with its current state.
- `set +o` output (POSIX format) shall include `set -o emacs` or `set +o emacs` as appropriate.

### 13.2 Startup Initialization

When emacs mode is enabled in an interactive shell, meiksh shall consult at most one inputrc file in the following order:

1. The file named by `$INPUTRC`, if that environment variable is set and non-empty.
2. `$HOME/.inputrc`, if `$HOME` is set.
3. `/etc/inputrc`.

- The first file that exists and is readable shall be parsed. Absence of all three shall not be an error.
- Parse errors shall be reported on standard error with the format `meiksh: <file>: line <n>: <diagnostic>` and shall not abort parsing; the parser shall skip to the next line boundary and continue.
- The inputrc file shall be parsed at most once per shell session unless explicitly re-read via `bind -f`.

### 13.3 History Integration

- The editor shall access the history list via the existing interfaces in `src/interactive/history.rs`. Accepting a line via `accept-line` shall not itself append to history; history accumulation is the shell REPL's responsibility.

## 14. `bind` Builtin

### 14.1 Synopsis

```
bind [-lpr] [-f filename] [-r keyseq] [-x keyseq:shell-command]
     [keyseq:function-name ...]
     [keyseq function-name]
```

The final two positional forms are mutually exclusive and are disambiguated per Section 14.5.

### 14.2 Description

The `bind` builtin shall inspect and modify the key-to-function bindings of the emacs keymap. All changes shall take effect on the next `read_line` invocation and shall persist for the life of the shell session.

### 14.3 Options

- **(no options, no arguments)**: list the current bindings in inputrc-compatible format on standard output.
- **`-l`**: list the names of all bindable functions, one per line, on standard output.
- **`-p`**: list current bindings in inputrc-compatible format (equivalent to the no-argument form but explicitly requested).
- **`-r keyseq`**: remove the binding for `keyseq`. If `keyseq` is not bound, exit status shall be nonzero; no diagnostic shall be printed.
- **`-f filename`**: read `filename` and apply its bindings as if each line were a `bind` argument. The file shall be parsed with the same grammar as `$HOME/.inputrc` (see [inputrc.md](inputrc.md)). Parse errors shall be reported as in Section 13.2 and shall not abort parsing.
- **`-x keyseq:shell-command`**: bind `keyseq` such that pressing the key sequence runs `shell-command` through `execute_string` in the current shell environment. The current buffer shall be saved before execution and restored afterwards; `shell-command` may read and modify the special variables `READLINE_LINE` and `READLINE_POINT` to interact with the editor state. Upon return, the editor shall redraw based on `READLINE_LINE` and `READLINE_POINT`. This form is required for `fzf`-style integrations.

### 14.4 Single-Argument And Multi-Argument Readline Form

- `bind keyseq:function-name` shall bind `keyseq` to the named bindable function.
- `bind "string"` (where `string` uses inputrc quoting) shall be parsed as a single inputrc line and applied.
- `bind arg1 arg2 ...` where every positional argument contains at least one `:` shall be processed by applying each argument independently as a separate inputrc line. Per-argument diagnostics shall be emitted in the `meiksh: -: line 1: ...` format. The overall exit status shall be 0 regardless of per-argument errors; this matches bash's observable behaviour.

### 14.5 Editline-Style Positional Form (portability)

For compatibility with `~/.shrc` files written for shells that use the editline/libedit line editor (notably FreeBSD `/bin/sh` and tcsh), `bind` shall additionally accept the two-positional-argument form

```
bind <keyseq> <function-name>
```

when **neither** positional argument contains `:`. This form applies only to the `bind` builtin invocation; inputrc files (Section 13, [inputrc.md](inputrc.md) § 4) continue to use the readline `keyseq: function-name` grammar exclusively and shall not recognize any of the editline function names below.

#### 14.5.1 Key sequence decoding

The `<keyseq>` argument shall accept, in addition to the escapes enumerated in [inputrc.md](inputrc.md) § 4.5:

- `^<c>` — control-letter notation. `^a`/`^A` → `0x01`, `^[` → `0x1b` (ESC), `^?` → `0x7f` (DEL). Any ASCII byte `c` is accepted and mapped by `c & 0x1f` after uppercasing, with `^?` as the special case for DEL.
- `\E` — ESC (`0x1b`). Editline accepts either `\e` or `\E`; readline-proper only accepts `\e`. `bind` accepts both.

A trailing `^` with no following byte, or a trailing `\` with no following byte, shall produce a `bind: dangling ... in key sequence` diagnostic and exit status 1.

#### 14.5.2 Function name translation

The `<function-name>` argument shall be resolved in the following order:

1. If the name is a readline canonical bindable-function name (see Section 5), it resolves to that function. This lets mixed-dialect input (`bind ^[[A previous-history`) work without surprise.
2. If the name is one of the editline function names in Table 14.5.2, it resolves to the mapped function.
3. If the name is a known editline-only function with no readline analogue, exit status 1 and diagnostic `bind: unsupported editline function: <name>`.
4. Otherwise, exit status 1 and diagnostic `bind: unknown function: <name>` (same wording as the readline path).

##### Table 14.5.2 — editline → readline function mapping

| Editline name | Readline function |
|---|---|
| `ed-search-prev-history` | `history-search-backward` |
| `ed-search-next-history` | `history-search-forward` |
| `ed-prev-history` | `previous-history` |
| `ed-next-history` | `next-history` |
| `em-inc-search-prev` | `reverse-search-history` |
| `em-inc-search-next` | `forward-search-history` |
| `ed-prev-char` | `backward-char` |
| `ed-next-char` | `forward-char` |
| `em-next-word` / `ed-next-word` | `forward-word` |
| `ed-prev-word` | `backward-word` |
| `ed-move-to-beg` | `beginning-of-line` |
| `ed-move-to-end` | `end-of-line` |
| `ed-delete-prev-char` | `backward-delete-char` |
| `ed-delete-next-char` | `delete-char` |
| `ed-delete-prev-word` / `em-delete-prev-word` | `backward-kill-word` |
| `em-kill-line` | `unix-line-discard` |
| `ed-kill-line` | `kill-line` |
| `em-yank` | `yank` |
| `ed-transpose-chars` | `transpose-chars` |
| `em-upper-case` | `upcase-word` |
| `em-lower-case` | `downcase-word` |
| `em-capitol-case` | `capitalize-word` |
| `ed-quoted-insert` | `quoted-insert` |
| `ed-clear-screen` | `clear-screen` |
| `ed-newline` | `accept-line` |
| `ed-insert` | `self-insert` |
| `em-undo` | `undo` |

Editline functions that are deliberately unsupported (diagnostic "unsupported editline function"): `vi-cmd-mode`, `vi-insert` (mid-line mode switching is a non-goal, Section 15.7); `em-set-mark`, `em-exchange-mark`, `em-kill-region`, `em-copy-region`, `em-copy-prev-word` (region/marks are a non-goal, Section 15.3); `em-toggle-overwrite` (Section 15.4); `em-universal-argument`, `em-argument-digit`, `em-meta-next` (numeric arguments are a non-goal, Section 15.5); `ed-redisplay`, `ed-refresh`, `ed-start-over` (no bindable redraw functions); `ed-list-choices`, `em-delete-or-list` (expanded completion set is a non-goal, Section 15.8); `ed-end-of-file`; every `ed-tty-*` signal passthrough (handled via native signal dispatch).

### 14.6 Exit Status

- 0: requested operation completed successfully, or multi-argument readline form (regardless of per-argument errors).
- 1: `-r` attempted to remove a non-existent binding, or a malformed `keyseq` was supplied, or `-f` encountered only errors, or an unknown function name was used, or the editline positional form failed to decode its keyseq / resolve its function name.
- 2: invalid option.

### 14.7 Notes

- Options not listed here (`-m`, `-q`, `-u`, `-s`, `-S`, `-v`, `-V`, `-X`, `-P`) are not accepted and shall produce exit status 2 with an "invalid option" diagnostic. Appendix B, Package 13 describes the work to add them.
- There is only a single keymap (`emacs`); `bind` shall not accept the `-m` option to target a specific keymap.

## 15. Non-Goals

The following features exist in one or more of the reference shells but are not part of this specification. Each absence is a deliberate choice; the related bindable functions shall not be provided and any `bind` attempt to reference them shall produce exit status 1.

### 15.1 Kill Ring And Yank-Pop

No kill ring; only a single kill buffer (Section 6). The functions `yank-pop`, `copy-region-as-kill`, `copy-backward-word`, `copy-forward-word`, and `kill-region` shall not be provided. See Appendix B, Package 1.

### 15.2 Keyboard Macros

The functions `start-kbd-macro` (`C-x (`), `end-kbd-macro` (`C-x )`), `call-last-kbd-macro` (`C-x e`), and `print-last-kbd-macro` shall not be provided. See Appendix B, Package 4.

### 15.3 Named Marks And Active Region

The functions `set-mark` (`C-@`), `exchange-point-and-mark` (`C-x C-x`), and the region-highlighting infrastructure shall not be provided. The inputrc variables `enable-active-region`, `active-region-start-color`, and `active-region-end-color` shall not be recognized. See Appendix B, Package 3.

### 15.4 Overwrite Mode

The function `overwrite-mode` shall not be provided. Each editing session shall start and remain in insert mode. See Appendix B, Package 8.

### 15.5 Numeric Arguments

The functions `digit-argument` (`M-0`..`M-9`), `universal-argument`, and the general "numeric prefix argument" concept shall not be provided. No bindable function takes a repeat count. See Appendix B, Package 2.

### 15.6 Non-Incremental History Search

The functions `non-incremental-reverse-search-history` (`M-p`), `non-incremental-forward-search-history` (`M-n`), and `history-substring-search-forward` / `-backward` shall not be provided. See Appendix B, Package 6.

### 15.7 Mid-Line Editing-Mode Switching

The functions `vi-editing-mode` (bindable to `C-M-j` in bash) and `emacs-editing-mode` shall not be provided. Users shall switch editing modes using `set -o emacs` and `set -o vi`, which take effect on the next input line. See Appendix B, Package 10.

### 15.8 Expanded Completion Set

The following bindable completion functions shall not be provided: `menu-complete`, `menu-complete-backward`, `delete-char-or-list`, `complete-filename`, `possible-filename-completions`, `complete-username`, `possible-username-completions`, `complete-hostname`, `possible-hostname-completions`, `complete-variable`, `possible-variable-completions`, `complete-command`, `possible-command-completions`, `dynamic-complete-history`, `dabbrev-expand`, `glob-complete-word`, `glob-expand-word`, `glob-list-expansions`, `tilde-expand` (on `M-&`), `tab-insert` (on `M-TAB`), `insert-completions` (on `M-*`), `possible-completions` (on `M-?`). The single `complete` function (Section 5.7) dispatches to the appropriate source based on position. See Appendix B, Package 5.

### 15.9 Character Search

The functions `character-search` (`C-]`) and `character-search-backward` (`M-C-]`) shall not be provided. See Appendix B, Package 7.

### 15.10 Dump Commands

The functions `dump-functions`, `dump-variables`, and `dump-macros` shall not be provided. Use `bind -p` and `bind -l` for equivalent information. See Appendix B, Package 9.

### 15.11 8-Bit Meta Input

Meiksh shall recognize meta only in its ESC-prefix form. Terminals configured to transmit 8-bit meta (high-bit-set bytes) shall be interpreted as UTF-8 continuation bytes or `self-insert` of the raw byte, not as meta keys.

The inputrc variables `input-meta` (and its readline alias `meta-flag`) and `output-meta` shall be accepted by the parser without diagnostic and their values shall be stored on the editor context, but those values shall have no runtime effect: the ESC-prefix rule above is unconditional. Accepting these variables exists solely so that distribution-shipped inputrc files (notably `/etc/inputrc` on Debian and Fedora) load without spurious `unknown variable` warnings.

The inputrc variables `convert-meta` and `enable-meta-key` shall not be recognized. See Appendix B, Package 12 for what it would take to give `input-meta` / `output-meta` / `convert-meta` / `enable-meta-key` real runtime effect.

### 15.12 Separate Keymaps

The keymap identifiers `emacs-meta` and `emacs-ctlx` shall not be user-visible. There is a single `emacs` keymap; `M-` and `C-x` prefixes are internal dispatch details. See Appendix B, Package 11.

### 15.13 Obscure Readline Variables

The following readline variables shall not be recognized by the inputrc parser or the `bind -v`-equivalent surface. Their presence in an inputrc file shall produce a per-line diagnostic but shall not abort parsing. `horizontal-scroll-mode`, `mark-modified-lines`, `page-completions`, `enable-keypad`, `skip-completed-text`, `menu-complete-display-prefix`, `completion-prefix-display-length`, `completion-display-width`, `visible-stats`, `print-completions-horizontally`, `blink-matching-paren`, `history-preserve-point`, `match-hidden-files`, `expand-tilde`, `revert-all-at-newline`, `isearch-terminators`, `show-mode-in-prompt`, `emacs-mode-string`, `vi-cmd-mode-string`, `vi-ins-mode-string`, `bind-tty-special-chars`, `echo-control-characters`, `disable-completion`, `search-ignore-case`, `colored-completion-prefix`, `active-region-start-color`, `active-region-end-color`. See Appendix B, Package 12.

### 15.14 `$if` Variants

The inputrc parser shall recognize `$if mode=emacs`, `$if mode=vi`, and `$if term=<name>`. The `term=<name>` test shall evaluate true when the `TERM` environment variable (captured at the time the inputrc was opened) equals `<name>` exactly or when the portion of `TERM` up to the first `-` equals `<name>`; otherwise it shall evaluate false. A well-formed `term=<name>` test that matches nothing shall not produce a diagnostic.

The variants `$if application=<name>`, `$if variable=value`, and `$if version>=<n>` shall not be recognized and shall produce per-line diagnostics. See Appendix B, Package 12.

### 15.15 `bind` Flags Beyond The Kept Set

See Section 14.7.

### 15.16 Menu Completion

No inline cycling of completions. Listing occurs on a second `TAB` (or first, if `show-all-if-ambiguous` is on). See Appendix B, Package 5.

### 15.17 Per-Terminal Bindings

Meiksh hardcodes the ANSI sequences emitted by xterm-class, screen-class, linux, and ansi terminals for arrow keys, `Home`, `End`, `Page Up`, `Page Down`, `Delete`, and F-keys. Terminals that emit different sequences shall require the user to rebind via inputrc; `$if term=<name>` (Section 15.14) is available for gating such rebinds.

## Appendix A - Comparison And Samples

### A.1 Comparison With Reference Shells

The table below summarizes the headline differences between meiksh emacs mode and the reference shells. "Same" means the default binding and behavior match meiksh. An entry other than "Same" indicates a deliberate difference.

| Feature | meiksh | bash 5.2 | ksh93 | zsh `bindkey -e` |
|---|---|---|---|---|
| Default on interactive startup | Off | On | On when `$VISUAL` / `$EDITOR` matches `*macs*` | On |
| `set -o emacs` / `set -o vi` mutually exclusive | Yes (normative) | Loosely, via `editing-mode` | Yes | Yes (linked keymaps) |
| Kill ring with `M-y` | No | Yes | No | Yes |
| Numeric arguments | No | Yes | No | Yes |
| Keyboard macros | No | Yes | No | Yes |
| Incremental search `C-r` | Yes | Yes | Yes | Yes |
| `history-search-backward` bindable | Yes (unbound by default) | Yes | No | Yes |
| `bind -x` | Yes | Yes | No | No (uses widgets) |
| `$if term=` | Yes | Yes | No | N/A |
| `bind` positional dialect | Readline-style and editline-style (`bind ^[[A ed-search-prev-history`) both accepted | Readline-style only | Readline-style only | N/A (uses `bindkey`) |

### A.2 Sample `~/.inputrc`

```text
# Search history by prefix with arrow keys
"\e[A": history-search-backward
"\e[B": history-search-forward

# Case-insensitive completion and listing
set completion-ignore-case on
set completion-map-case on
set show-all-if-ambiguous on

# No audible bell
set bell-style none

# Bracketed paste is on by default; kept here as a comment for discoverability
# set enable-bracketed-paste on
```

## Appendix B - Path To Full Readline Parity

This appendix is non-normative. It describes what it would take to lift the current subset to full bash/readline emacs-mode parity. The cuts documented in Section 15 are grouped here into work packages sized from "tiny" (a single function and a test) to "large" (substantial changes to the dispatch core or completion layer).

### B.1 Package 1 - Kill Ring And `yank-pop`

**Effort**: small. **Dependencies**: none.

Replace the single-buffer kill model with a bounded ring of buffers. Add the `yank-pop` function (`M-y`), which shall only be valid immediately after `yank` or another `yank-pop` and rotates the ring, replacing the most recently yanked text with the new top. Cuts addressed: 15.1.

### B.2 Package 2 - Numeric Arguments

**Effort**: medium. **Dependencies**: none, but touches every bindable function.

Add `digit-argument` and `universal-argument`. Plumb a `count: i64` parameter through every bindable function so that "`M-4 C-k`" kills four lines forward. Argument termination rules match bash: an unbindable digit or other non-argument key terminates the accumulation and invokes the underlying function with the accumulated count. Cuts addressed: 15.5.

### B.3 Package 3 - Named Marks And Active Region

**Effort**: medium. **Dependencies**: none.

Add a `mark: usize` field alongside `cursor`. Implement `set-mark`, `exchange-point-and-mark`, `kill-region`, `copy-region-as-kill`, `copy-backward-word`, `copy-forward-word`. Implement region-activation tracking and, gated on terminal color support and the `enable-active-region` variable, colored display of the region using ANSI SGR sequences driven by `active-region-start-color` and `active-region-end-color`. Cuts addressed: 15.1 (partial: the `-region` and `copy-*` functions), 15.3.

### B.4 Package 4 - Keyboard Macros

**Effort**: small for dispatch; moderate for the record/replay state machine.

Add `start-kbd-macro`, `end-kbd-macro`, `call-last-kbd-macro`, `print-last-kbd-macro`. Recording stores every keystroke to a buffer; replay feeds the buffer through the dispatch loop. Interaction with bracketed paste and undo is specified in the bash readline manual and shall be matched. Cuts addressed: 15.2.

### B.5 Package 5 - Expanded Completion Set

**Effort**: large. **Dependencies**: Section 5.8 completion core.

Implement the full set of `complete-*` / `possible-*` / glob / history / dabbrev / tilde functions. Implement `menu-complete` and `menu-complete-backward` as an in-place cycling state machine that replaces the partial word with each candidate in turn, reverting on any non-menu-complete key. Each function needs its own completion source, honoring all the readline variables in Package 12. Cuts addressed: 15.8, 15.16.

### B.6 Package 6 - Non-Incremental History Search

**Effort**: small. **Dependencies**: history scan is already present.

Add `non-incremental-reverse-search-history` (`M-p`), `non-incremental-forward-search-history` (`M-n`), `history-substring-search-forward`, `history-substring-search-backward`. Cuts addressed: 15.6.

### B.7 Package 7 - Character Search

**Effort**: tiny.

Add `character-search` (`C-]`) and `character-search-backward` (`M-C-]`). Each reads one more byte and moves the cursor to the next or previous occurrence of that byte in the buffer. Cuts addressed: 15.9.

### B.8 Package 8 - Overwrite Mode

**Effort**: tiny.

Add `overwrite-mode` function. `self-insert` when overwrite is on replaces the character at the cursor rather than inserting before it. At end-of-buffer, overwrite behaves identically to insert. Cuts addressed: 15.4.

### B.9 Package 9 - Dump Commands

**Effort**: small.

Add `dump-functions`, `dump-variables`, `dump-macros`. Each writes its respective state to the editor's output stream in inputrc-compatible format. Cuts addressed: 15.10.

### B.10 Package 10 - Mid-Line Editing-Mode Switch

**Effort**: small, but requires restructuring.

Add `vi-editing-mode` (bindable to `C-M-j`) and `emacs-editing-mode`. Switching mid-line requires the vi and emacs editors to share a single buffer/undo/kill state, rather than being selected once per `read_line`. This is the one structural change among "small" packages. Cuts addressed: 15.7.

### B.11 Package 11 - Full Keymap Model

**Effort**: medium. **Dependencies**: none, but a prerequisite for Package 13.

Replace the current single-keymap dispatch with a keymap registry keyed by name. Expose `emacs-meta` and `emacs-ctlx` as first-class keymaps addressable via `bind -m`. Allow user-created keymaps via `bind -m newname`. Allow keymap aliases. Cuts addressed: 15.12.

### B.12 Package 12 - Full Inputrc Parser

**Effort**: large. **Dependencies**: Package 11 for keymap-based `$if keymap=`.

Add `$if application=`, `$if variable=value`, and `$if version>=<n>` (`$if term=` and `$if mode=` are already implemented). Add all ~25 cut readline variables with their defaults and editor-side effects. Give `input-meta`, `output-meta`, `convert-meta`, and `enable-meta-key` real runtime effect — today `input-meta` / `output-meta` are parsed and stored but ignored, and `convert-meta` / `enable-meta-key` are still unrecognized. Add `enable-keypad`. Add `isearch-terminators`. Add `show-mode-in-prompt` with `emacs-mode-string` / `vi-cmd-mode-string` / `vi-ins-mode-string`. Most variables require editor-side work, not just parsing. Cuts addressed: 15.11, 15.13, 15.14.

### B.13 Package 13 - Full `bind` Builtin

**Effort**: small. **Dependencies**: Package 11.

Add `-m`, `-q`, `-u`, `-s`, `-S`, `-v`, `-V`, `-X`, `-P`. These are mostly introspection built on top of the keymap registry. Cuts addressed: 15.15.

### B.14 Batching Recommendations

- **Independent drop-ins**: Packages 1, 2, 4, 6, 7, 8, 9. Each may be implemented and merged without touching the others.
- **Structural cluster**: Packages 3, 10, 11, 12. These interact (active region needs color variables from Package 12; mid-line switching benefits from the keymap registry in Package 11; the full parser needs Package 11 for `$if keymap=`) and should be batched in that order.
- **Largest single prerequisite for user-flexible rebinding**: Package 11. Investing in the keymap model first unlocks Package 13 cheaply.

## Appendix C - Full Cut List

This appendix restates Section 15's cuts as a flat enumeration, cross-referenced to Appendix B. The non-goal list is normative (Section 15); this appendix is for discoverability only.

| Section | Cut item | Restored by |
|---|---|---|
| 15.1 | Kill ring and `M-y` yank-pop | Package 1 |
| 15.1 | `copy-region-as-kill`, `copy-backward-word`, `copy-forward-word`, `kill-region` | Packages 1, 3 |
| 15.2 | Keyboard macros (`C-x (`, `C-x )`, `C-x e`, `print-last-kbd-macro`) | Package 4 |
| 15.3 | Named marks (`set-mark` `C-@`, `exchange-point-and-mark` `C-x C-x`) | Package 3 |
| 15.3 | Active region and color variables | Packages 3, 12 |
| 15.4 | Overwrite mode | Package 8 |
| 15.5 | Numeric arguments (`M-<digit>`, `M--`, `universal-argument`) | Package 2 |
| 15.6 | Non-incremental history search (`M-p`, `M-n`) | Package 6 |
| 15.6 | History substring search | Package 6 |
| 15.7 | Mid-line `vi-editing-mode` / `emacs-editing-mode` widgets | Package 10 |
| 15.8 | `menu-complete`, `menu-complete-backward`, `delete-char-or-list` | Package 5 |
| 15.8 | `complete-filename` / `-username` / `-hostname` / `-variable` / `-command` and `possible-*` twins | Package 5 |
| 15.8 | `dynamic-complete-history`, `dabbrev-expand` | Package 5 |
| 15.8 | `glob-complete-word`, `glob-expand-word`, `glob-list-expansions` | Package 5 |
| 15.8 | `tilde-expand` on `M-&`, `tab-insert` on `M-TAB`, `insert-completions` on `M-*`, `possible-completions` on `M-?` | Package 5 |
| 15.9 | `character-search`, `character-search-backward` | Package 7 |
| 15.10 | `dump-functions`, `dump-variables`, `dump-macros` | Package 9 |
| 15.11 | 8-bit meta runtime effect for `input-meta`, `output-meta` (parsed but ignored); full `convert-meta`, `enable-meta-key` support | Package 12 |
| 15.12 | `emacs-meta`, `emacs-ctlx` as user-visible keymaps, user-created keymaps | Package 11 |
| 15.13 | ~25 obscure readline variables | Package 12 |
| 15.14 | `$if application=`, `$if variable=`, `$if version>=` (`$if term=` and `$if mode=` are implemented) | Package 12 |
| 15.15 | `bind` flags `-m`, `-q`, `-u`, `-s`, `-S`, `-v`, `-V`, `-X`, `-P` | Packages 11, 13 |
| 15.16 | Inline menu completion cycling | Package 5 |
| 15.17 | Per-terminal bindings via `$if term=` (implemented; terminals outside the xterm/screen/linux/ansi set still need explicit rebinds) | Package 12 |
