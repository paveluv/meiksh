//! Emacs keymap: a byte-level prefix trie mapping key sequences to
//! bindable entries.
//!
//! Each node holds an optional [`KeymapEntry`] plus a child-node map
//! keyed by raw input byte. Resolution over an in-flight sequence
//! returns [`Resolved`]::`NeedsMore` whenever the prefix matches an
//! internal node and [`Resolved`]::`Unbound` when no child matches.
//!
//! The default table encodes the bindings specified in
//! [`docs/features/emacs-editing-mode.md`] § 5. Tests exercise a
//! small cross-section; the integration tests in Stage B exercise
//! the full table end-to-end over a real PTY.

use std::collections::HashMap;

/// The finite set of bindable emacs functions. Names match the
/// spec / GNU Readline conventions (kebab-case rendered by
/// [`EmacsFn::name`]).
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum EmacsFn {
    SelfInsert,
    BeginningOfLine,
    EndOfLine,
    ForwardChar,
    BackwardChar,
    ForwardWord,
    BackwardWord,
    ClearScreen,
    PreviousHistory,
    NextHistory,
    ReverseSearchHistory,
    ForwardSearchHistory,
    YankLastArg,
    HistorySearchBackward,
    HistorySearchForward,
    BeginningOfHistory,
    EndOfHistory,
    BackwardDeleteChar,
    DeleteChar,
    KillLine,
    UnixLineDiscard,
    UnixWordRubout,
    KillWord,
    BackwardKillWord,
    Yank,
    TransposeChars,
    TransposeWords,
    UpcaseWord,
    DowncaseWord,
    CapitalizeWord,
    QuotedInsert,
    Complete,
    AcceptLine,
    Undo,
    Abort,
    SendSigint,
    EditAndExecuteCommand,
}

impl EmacsFn {
    /// Kebab-case string used in inputrc and by `bind -l` output.
    pub(crate) fn name(self) -> &'static [u8] {
        use EmacsFn::*;
        match self {
            SelfInsert => b"self-insert",
            BeginningOfLine => b"beginning-of-line",
            EndOfLine => b"end-of-line",
            ForwardChar => b"forward-char",
            BackwardChar => b"backward-char",
            ForwardWord => b"forward-word",
            BackwardWord => b"backward-word",
            ClearScreen => b"clear-screen",
            PreviousHistory => b"previous-history",
            NextHistory => b"next-history",
            ReverseSearchHistory => b"reverse-search-history",
            ForwardSearchHistory => b"forward-search-history",
            YankLastArg => b"yank-last-arg",
            HistorySearchBackward => b"history-search-backward",
            HistorySearchForward => b"history-search-forward",
            BeginningOfHistory => b"beginning-of-history",
            EndOfHistory => b"end-of-history",
            BackwardDeleteChar => b"backward-delete-char",
            DeleteChar => b"delete-char",
            KillLine => b"kill-line",
            UnixLineDiscard => b"unix-line-discard",
            UnixWordRubout => b"unix-word-rubout",
            KillWord => b"kill-word",
            BackwardKillWord => b"backward-kill-word",
            Yank => b"yank",
            TransposeChars => b"transpose-chars",
            TransposeWords => b"transpose-words",
            UpcaseWord => b"upcase-word",
            DowncaseWord => b"downcase-word",
            CapitalizeWord => b"capitalize-word",
            QuotedInsert => b"quoted-insert",
            Complete => b"complete",
            AcceptLine => b"accept-line",
            Undo => b"undo",
            Abort => b"abort",
            SendSigint => b"send-sigint",
            EditAndExecuteCommand => b"edit-and-execute-command",
        }
    }

    /// Reverse lookup: resolve a kebab-case function name, rejecting
    /// unknown names (inputrc / `bind` emit a diagnostic instead of
    /// installing the binding).
    pub(crate) fn from_name(name: &[u8]) -> Option<Self> {
        ALL_FUNCTIONS.iter().copied().find(|f| f.name() == name)
    }
}

/// Canonical ordering of every bindable function. Used by `bind -l`
/// and by `EmacsFn::from_name`.
pub(crate) const ALL_FUNCTIONS: &[EmacsFn] = &[
    EmacsFn::AcceptLine,
    EmacsFn::BackwardChar,
    EmacsFn::BackwardDeleteChar,
    EmacsFn::BackwardKillWord,
    EmacsFn::BackwardWord,
    EmacsFn::BeginningOfHistory,
    EmacsFn::BeginningOfLine,
    EmacsFn::CapitalizeWord,
    EmacsFn::ClearScreen,
    EmacsFn::Complete,
    EmacsFn::DeleteChar,
    EmacsFn::DowncaseWord,
    EmacsFn::EditAndExecuteCommand,
    EmacsFn::EndOfHistory,
    EmacsFn::EndOfLine,
    EmacsFn::ForwardChar,
    EmacsFn::ForwardSearchHistory,
    EmacsFn::ForwardWord,
    EmacsFn::HistorySearchBackward,
    EmacsFn::HistorySearchForward,
    EmacsFn::KillLine,
    EmacsFn::KillWord,
    EmacsFn::NextHistory,
    EmacsFn::PreviousHistory,
    EmacsFn::QuotedInsert,
    EmacsFn::ReverseSearchHistory,
    EmacsFn::Abort,
    EmacsFn::SelfInsert,
    EmacsFn::SendSigint,
    EmacsFn::TransposeChars,
    EmacsFn::TransposeWords,
    EmacsFn::Undo,
    EmacsFn::UnixLineDiscard,
    EmacsFn::UnixWordRubout,
    EmacsFn::UpcaseWord,
    EmacsFn::Yank,
    EmacsFn::YankLastArg,
];

/// A leaf value in the keymap trie.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum KeymapEntry {
    Func(EmacsFn),
    Macro(Vec<u8>),
    ExecShell(Vec<u8>),
}

/// Result of a sequence-lookup in the trie.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Resolved {
    /// The bytes matched a terminal leaf.
    Function(EmacsFn),
    /// The bytes matched a leaf whose entry is a macro expansion.
    Macro(Vec<u8>),
    /// The bytes matched a leaf bound to an external shell command.
    ExecShell(Vec<u8>),
    /// The bytes are a proper prefix of a longer binding; the caller
    /// should read more input (subject to `keyseq-timeout`).
    NeedsMore,
    /// No binding matches.
    Unbound,
}

#[derive(Clone, Debug, Default)]
struct Node {
    entry: Option<KeymapEntry>,
    children: HashMap<u8, Node>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Keymap {
    root: Node,
}

impl Keymap {
    /// Build the emacs default keymap from the spec's § 5 tables.
    pub(crate) fn default_emacs() -> Self {
        let mut km = Self::default();
        for (seq, entry) in DEFAULT_BINDINGS {
            km.bind(seq, KeymapEntry::Func(*entry));
        }
        km
    }

    /// Bind `keyseq` to `entry`, replacing any prior binding.
    pub(crate) fn bind(&mut self, keyseq: &[u8], entry: KeymapEntry) {
        if keyseq.is_empty() {
            return;
        }
        let mut node = &mut self.root;
        for b in keyseq {
            node = node.children.entry(*b).or_default();
        }
        node.entry = Some(entry);
    }

    /// Remove the binding at `keyseq`. Returns `true` if a binding was
    /// removed, `false` if the sequence was never bound.
    pub(crate) fn unbind(&mut self, keyseq: &[u8]) -> bool {
        fn walk(node: &mut Node, keyseq: &[u8]) -> bool {
            if keyseq.is_empty() {
                if node.entry.is_some() {
                    node.entry = None;
                    return true;
                }
                return false;
            }
            let Some(child) = node.children.get_mut(&keyseq[0]) else {
                return false;
            };
            let removed = walk(child, &keyseq[1..]);
            if child.entry.is_none() && child.children.is_empty() {
                node.children.remove(&keyseq[0]);
            }
            removed
        }
        walk(&mut self.root, keyseq)
    }

    /// Resolve a complete (or partial) key sequence.
    pub(crate) fn resolve(&self, bytes: &[u8]) -> Resolved {
        let mut node = &self.root;
        for b in bytes {
            match node.children.get(b) {
                Some(child) => node = child,
                None => return Resolved::Unbound,
            }
        }
        match &node.entry {
            Some(KeymapEntry::Func(f)) => Resolved::Function(*f),
            Some(KeymapEntry::Macro(b)) => Resolved::Macro(b.clone()),
            Some(KeymapEntry::ExecShell(b)) => Resolved::ExecShell(b.clone()),
            None => {
                if node.children.is_empty() {
                    Resolved::Unbound
                } else {
                    Resolved::NeedsMore
                }
            }
        }
    }

    /// Borrow every installed binding in `(keyseq, entry)` form. Used
    /// by `bind -p` and `Keymap::dump_inputrc`.
    pub(crate) fn bindings(&self) -> Vec<(Vec<u8>, KeymapEntry)> {
        let mut out = Vec::new();
        collect(&self.root, &mut Vec::new(), &mut out);
        out.sort_by(|(a, _), (b, _)| a.cmp(b));
        out
    }

    /// Serialize the current keymap in inputrc-compatible format
    /// (`"keyseq": function-name` / `"keyseq": "macro"`).
    pub(crate) fn dump_inputrc(&self, out: &mut Vec<u8>) {
        for (seq, entry) in self.bindings() {
            out.push(b'"');
            write_escaped(&seq, out);
            out.extend_from_slice(b"\": ");
            match entry {
                KeymapEntry::Func(f) => out.extend_from_slice(f.name()),
                KeymapEntry::Macro(bytes) => {
                    out.push(b'"');
                    write_escaped(&bytes, out);
                    out.push(b'"');
                }
                KeymapEntry::ExecShell(bytes) => {
                    out.extend_from_slice(b"bind-exec \"");
                    write_escaped(&bytes, out);
                    out.push(b'"');
                }
            }
            out.push(b'\n');
        }
    }
}

fn collect(node: &Node, prefix: &mut Vec<u8>, out: &mut Vec<(Vec<u8>, KeymapEntry)>) {
    if let Some(entry) = &node.entry {
        out.push((prefix.clone(), entry.clone()));
    }
    for (byte, child) in &node.children {
        prefix.push(*byte);
        collect(child, prefix, out);
        prefix.pop();
    }
}

fn write_escaped(bytes: &[u8], out: &mut Vec<u8>) {
    for &b in bytes {
        match b {
            0x1b => out.extend_from_slice(b"\\e"),
            b'"' => out.extend_from_slice(b"\\\""),
            b'\\' => out.extend_from_slice(b"\\\\"),
            b if b < 0x20 => {
                out.push(b'\\');
                out.push(b'C');
                out.push(b'-');
                out.push(b + b'a' - 1);
            }
            b if b < 0x7f => out.push(b),
            b => {
                out.push(b'\\');
                out.push(b'x');
                out.push(hex_digit((b >> 4) & 0x0f));
                out.push(hex_digit(b & 0x0f));
            }
        }
    }
}

fn hex_digit(nibble: u8) -> u8 {
    match nibble {
        0..=9 => b'0' + nibble,
        _ => b'a' + nibble - 10,
    }
}

/// Default emacs bindings per spec § 5. The byte sequences are the
/// raw bytes a VT-style terminal actually emits; `\x1b` = ESC, etc.
pub(crate) const DEFAULT_BINDINGS: &[(&[u8], EmacsFn)] = &[
    // --- control characters (section 5.2) ---
    (b"\x01", EmacsFn::BeginningOfLine),      // C-a
    (b"\x02", EmacsFn::BackwardChar),         // C-b
    (b"\x03", EmacsFn::SendSigint),           // C-c
    (b"\x04", EmacsFn::DeleteChar),           // C-d
    (b"\x05", EmacsFn::EndOfLine),            // C-e
    (b"\x06", EmacsFn::ForwardChar),          // C-f
    (b"\x07", EmacsFn::Abort),                // C-g
    (b"\x08", EmacsFn::BackwardDeleteChar),   // C-h
    (b"\x09", EmacsFn::Complete),             // TAB
    (b"\x0a", EmacsFn::AcceptLine),           // C-j
    (b"\x0b", EmacsFn::KillLine),             // C-k
    (b"\x0c", EmacsFn::ClearScreen),          // C-l
    (b"\x0d", EmacsFn::AcceptLine),           // RET (C-m)
    (b"\x0e", EmacsFn::NextHistory),          // C-n
    (b"\x10", EmacsFn::PreviousHistory),      // C-p
    (b"\x11", EmacsFn::QuotedInsert),         // C-q
    (b"\x12", EmacsFn::ReverseSearchHistory), // C-r
    (b"\x13", EmacsFn::ForwardSearchHistory), // C-s
    (b"\x14", EmacsFn::TransposeChars),       // C-t
    (b"\x15", EmacsFn::UnixLineDiscard),      // C-u
    (b"\x16", EmacsFn::QuotedInsert),         // C-v
    (b"\x17", EmacsFn::UnixWordRubout),       // C-w
    (b"\x19", EmacsFn::Yank),                 // C-y
    (b"\x1f", EmacsFn::Undo),                 // C-_
    (b"\x7f", EmacsFn::BackwardDeleteChar),   // DEL
    // --- ESC-prefixed (M-x) bindings, section 5.3 ---
    (b"\x1bf", EmacsFn::ForwardWord),           // M-f
    (b"\x1bb", EmacsFn::BackwardWord),          // M-b
    (b"\x1bd", EmacsFn::KillWord),              // M-d
    (b"\x1b\x7f", EmacsFn::BackwardKillWord),   // M-DEL
    (b"\x1b\x08", EmacsFn::BackwardKillWord),   // M-BS
    (b"\x1bu", EmacsFn::UpcaseWord),            // M-u
    (b"\x1bl", EmacsFn::DowncaseWord),          // M-l
    (b"\x1bc", EmacsFn::CapitalizeWord),        // M-c
    (b"\x1bt", EmacsFn::TransposeWords),        // M-t
    (b"\x1b.", EmacsFn::YankLastArg),           // M-.
    (b"\x1b_", EmacsFn::YankLastArg),           // M-_
    (b"\x1b<", EmacsFn::BeginningOfHistory),    // M-<
    (b"\x1b>", EmacsFn::EndOfHistory),          // M->
    (b"\x1bp", EmacsFn::HistorySearchBackward), // M-p
    (b"\x1bn", EmacsFn::HistorySearchForward),  // M-n
    (b"\x1br", EmacsFn::Abort),                 // M-r
    // --- Arrow keys: both CSI and SS3 forms (section 5.9) ---
    (b"\x1b[A", EmacsFn::PreviousHistory),
    (b"\x1b[B", EmacsFn::NextHistory),
    (b"\x1b[C", EmacsFn::ForwardChar),
    (b"\x1b[D", EmacsFn::BackwardChar),
    (b"\x1bOA", EmacsFn::PreviousHistory),
    (b"\x1bOB", EmacsFn::NextHistory),
    (b"\x1bOC", EmacsFn::ForwardChar),
    (b"\x1bOD", EmacsFn::BackwardChar),
    (b"\x1b[H", EmacsFn::BeginningOfLine),
    (b"\x1b[F", EmacsFn::EndOfLine),
    (b"\x1bOH", EmacsFn::BeginningOfLine),
    (b"\x1bOF", EmacsFn::EndOfLine),
    (b"\x1b[1~", EmacsFn::BeginningOfLine),
    (b"\x1b[4~", EmacsFn::EndOfLine),
    (b"\x1b[3~", EmacsFn::DeleteChar),
    (b"\x1b[5~", EmacsFn::BeginningOfHistory), // PageUp
    (b"\x1b[6~", EmacsFn::EndOfHistory),       // PageDown
    (b"\x1b[1;5C", EmacsFn::ForwardWord),      // Ctrl+Right
    (b"\x1b[1;5D", EmacsFn::BackwardWord),     // Ctrl+Left
    // --- C-x prefix (section 5.5) ---
    (b"\x18\x05", EmacsFn::EditAndExecuteCommand), // C-x C-e
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::test_support::assert_no_syscalls;

    #[test]
    fn default_binds_c_a_to_beginning_of_line() {
        assert_no_syscalls(|| {
            let k = Keymap::default_emacs();
            assert_eq!(
                k.resolve(b"\x01"),
                Resolved::Function(EmacsFn::BeginningOfLine)
            );
            assert_eq!(k.resolve(b"\x05"), Resolved::Function(EmacsFn::EndOfLine));
        });
    }

    #[test]
    fn arrow_sequence_needs_more_then_function() {
        assert_no_syscalls(|| {
            let k = Keymap::default_emacs();
            assert_eq!(k.resolve(b"\x1b"), Resolved::NeedsMore);
            assert_eq!(k.resolve(b"\x1b["), Resolved::NeedsMore);
            assert_eq!(
                k.resolve(b"\x1b[A"),
                Resolved::Function(EmacsFn::PreviousHistory)
            );
        });
    }

    #[test]
    fn bind_overrides_default() {
        assert_no_syscalls(|| {
            let mut k = Keymap::default_emacs();
            k.bind(b"\x01", KeymapEntry::Func(EmacsFn::EndOfLine));
            assert_eq!(k.resolve(b"\x01"), Resolved::Function(EmacsFn::EndOfLine));
        });
    }

    #[test]
    fn bind_macro_and_exec_shell() {
        assert_no_syscalls(|| {
            let mut k = Keymap::default_emacs();
            k.bind(b"\x18g", KeymapEntry::Macro(b"git status\n".to_vec()));
            k.bind(b"\x18s", KeymapEntry::ExecShell(b"echo hi".to_vec()));
            match k.resolve(b"\x18g") {
                Resolved::Macro(v) => assert_eq!(v, b"git status\n"),
                other => panic!("unexpected: {other:?}"),
            }
            match k.resolve(b"\x18s") {
                Resolved::ExecShell(v) => assert_eq!(v, b"echo hi"),
                other => panic!("unexpected: {other:?}"),
            }
        });
    }

    #[test]
    fn unbind_reports_missing() {
        assert_no_syscalls(|| {
            let mut k = Keymap::default_emacs();
            assert!(k.unbind(b"\x01"));
            assert_eq!(k.resolve(b"\x01"), Resolved::Unbound);
            assert!(!k.unbind(b"\xff"));
        });
    }

    #[test]
    fn needs_more_for_proper_prefix() {
        assert_no_syscalls(|| {
            let k = Keymap::default_emacs();
            assert_eq!(k.resolve(b"\x18"), Resolved::NeedsMore);
        });
    }

    #[test]
    fn unbound_for_leaf_without_entry() {
        assert_no_syscalls(|| {
            let k = Keymap::default_emacs();
            assert_eq!(k.resolve(b"z"), Resolved::Unbound);
            assert_eq!(k.resolve(b""), Resolved::NeedsMore);
        });
    }

    #[test]
    fn dump_inputrc_round_trips_every_binding() {
        assert_no_syscalls(|| {
            let k = Keymap::default_emacs();
            let mut out = Vec::new();
            k.dump_inputrc(&mut out);
            // Every function name should appear at least once.
            for f in ALL_FUNCTIONS {
                // Skip self-insert — it's not assigned a dedicated
                // keyseq in the default table (self-insert is the
                // fallback for unbound printable bytes).
                if *f == EmacsFn::SelfInsert {
                    continue;
                }
                assert!(
                    out.windows(f.name().len()).any(|w| w == f.name()),
                    "dump missing function name: {}",
                    std::str::from_utf8(f.name()).unwrap(),
                );
            }
        });
    }

    #[test]
    fn bind_ignores_empty_keyseq() {
        // Binding the empty sequence is a no-op: nothing is ever
        // typed as "no bytes", so the trie rejects it silently.
        assert_no_syscalls(|| {
            let mut k = Keymap::default();
            k.bind(b"", KeymapEntry::Func(EmacsFn::AcceptLine));
            // The root is still empty: no entry and no children. The
            // resolver walks zero bytes, lands on the root, and
            // reports `Unbound` because both the entry is `None` and
            // the children map is empty.
            assert_eq!(k.resolve(b""), Resolved::Unbound);
        });
    }

    #[test]
    fn unbind_empty_keyseq_without_entry_returns_false() {
        // Walking with an empty keyseq into a node whose `entry` is
        // `None` hits the `return false` arm: nothing was bound at
        // the target, so the caller observes `false`.
        assert_no_syscalls(|| {
            let mut k = Keymap::default();
            assert!(!k.unbind(b""));
        });
    }

    #[test]
    fn dump_inputrc_round_trips_macro_binding() {
        // The macro branch of `dump_inputrc` writes `"seq": "macro"`
        // with escape handling for embedded quotes and backslashes.
        assert_no_syscalls(|| {
            let mut k = Keymap::default();
            k.bind(b"\x18m", KeymapEntry::Macro(b"a\"b\\c".to_vec()));
            let mut out = Vec::new();
            k.dump_inputrc(&mut out);
            // Expect the macro value to be quoted with the embedded
            // `"` and `\` escaped per `write_escaped`.
            assert!(
                out.windows(b"\"a\\\"b\\\\c\"".len())
                    .any(|w| w == b"\"a\\\"b\\\\c\""),
                "macro dump missing escape-preserving form: {:?}",
                String::from_utf8_lossy(&out),
            );
        });
    }

    #[test]
    fn emacs_fn_name_round_trip() {
        assert_no_syscalls(|| {
            for f in ALL_FUNCTIONS {
                assert_eq!(EmacsFn::from_name(f.name()), Some(*f));
            }
            assert_eq!(EmacsFn::from_name(b"no-such-function"), None);
        });
    }
}
