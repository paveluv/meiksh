//! Editline / tcsh dialect support for the `bind` builtin.
//!
//! FreeBSD (and older tcsh-era) shell init files use the positional
//! form `bind <keyseq> <function-name>` — for example
//!
//! ```text
//! bind ^[[A ed-search-prev-history
//! bind "\e[1;5C" em-next-word
//! ```
//!
//! Meiksh otherwise follows readline/bash, where `bind` takes a
//! single inputrc-format string (`bind '"\eOA": history-search-
//! backward'`). This module translates between the two dialects so
//! FreeBSD `~/.shrc` and similar dotfiles stop producing diagnostics
//! on startup. Scope is strictly the `bind` builtin — inputrc files
//! remain pure readline (`docs/features/inputrc.md` § 4).
//!
//! Two pieces are provided:
//!
//! * [`decode_editline_keyseq`] — decode a keyseq argument with
//!   editline's relaxed escape vocabulary: `^X` for control letters
//!   (including `^[` = ESC and `^?` = DEL), `\e` **and** `\E` for
//!   ESC (readline only recognises `\e`), plus the standard
//!   backslash escapes shared with `escape.rs`.
//! * [`translate_editline_function`] — map an editline function
//!   name to an [`EmacsFn`] if we have a clean readline equivalent,
//!   or classify the miss as "known editline function with no
//!   readline analogue" vs. "typo / unrecognised name" so the
//!   dispatcher can emit the right diagnostic.

#![allow(dead_code)]

use super::super::emacs_editing::keymap::EmacsFn;
use super::escape::{decode_escape, decode_quoted};

/// Outcome of looking up a function name in the editline dispatcher.
///
/// Separating `Unsupported` from `Unknown` lets the caller emit a
/// friendlier diagnostic: users pasting an editline-idiomatic name
/// (`vi-cmd-mode`) see "unsupported editline function", whereas
/// typos (`edd-search-prev-history`) see "unknown function", the
/// same wording the readline path uses.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum EditlineLookup {
    Mapped(EmacsFn),
    Unsupported,
    Unknown,
}

/// Decode a keyseq argument written in editline/tcsh conventions.
///
/// Supported sequences, in addition to plain literal bytes:
///
/// * `^<c>` — control-letter. `^a`/`^A` → 0x01, `^[` → 0x1b, `^?`
///   → 0x7f. Any ASCII byte `c` is accepted and mapped by
///   `c & 0x1f` (with the `^?` special case). A trailing `^` is a
///   decode error.
/// * `\e`, `\E` — ESC (0x1b). Readline only accepts the lowercase
///   form; editline/tcsh accept either.
/// * `\a \b \d \f \n \r \t \v \\ \" \'` — standard escapes, same
///   bytes as [`decode_escape`].
/// * `\NNN` — 1-3 octal digits, 0-0377.
/// * `\xNN` / `\XNN` — 1-2 hex digits.
/// * `\C-X`, `\M-X` — readline escapes are accepted unchanged so
///   mixed-dialect users aren't surprised.
///
/// Any byte that is neither `^` nor the start of a valid escape is
/// emitted literally. This tolerates the spade of ESC-bracket CSI
/// sequences (`^[[A`, `\e[1;5C`, etc.) found in real init files.
pub(crate) fn decode_editline_keyseq(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'^' => {
                if i + 1 >= bytes.len() {
                    return Err("dangling `^` in key sequence".to_string());
                }
                let c = bytes[i + 1];
                let byte = if c == b'?' {
                    0x7f
                } else {
                    c.to_ascii_uppercase() & 0x1f
                };
                out.push(byte);
                i += 2;
            }
            b'\\' => {
                if i + 1 >= bytes.len() {
                    return Err("dangling backslash in key sequence".to_string());
                }
                // `\E` is the editline synonym of `\e`. Everything else
                // delegates to the shared readline decoder so the byte
                // meanings stay in sync.
                if bytes[i + 1] == b'E' {
                    out.push(0x1b);
                    i += 2;
                } else {
                    let (decoded, step) = decode_escape(&bytes[i + 1..])?;
                    out.extend_from_slice(&decoded);
                    i += 1 + step;
                }
            }
            b'"' => {
                // Tolerate an unescaped double-quoted run inside the
                // argument — users sometimes write `bind "\e[1;5C" fn`
                // and the shell hands us the contents with the quotes
                // stripped, but a leftover literal `"` is not unheard
                // of. Consume bytes up to the closing quote using the
                // readline decoder so escapes keep their meaning.
                let (decoded, consumed) = decode_quoted(&bytes[i + 1..])?;
                out.extend_from_slice(&decoded);
                i += 1 + consumed;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    if out.is_empty() {
        return Err("empty key sequence".to_string());
    }
    Ok(out)
}

/// Map an editline / tcsh function name to meiksh's equivalent
/// [`EmacsFn`], or classify the miss. Readline canonical names are
/// also accepted so mixed-dialect input (`bind ^[[A previous-history`)
/// Just Works.
pub(crate) fn translate_editline_function(name: &[u8]) -> EditlineLookup {
    if let Some(func) = EmacsFn::from_name(name) {
        return EditlineLookup::Mapped(func);
    }
    for (editline_name, func) in MAPPING_TABLE {
        if *editline_name == name {
            return EditlineLookup::Mapped(*func);
        }
    }
    if UNSUPPORTED_NAMES.iter().any(|n| *n == name) || is_ed_tty_family(name) {
        return EditlineLookup::Unsupported;
    }
    EditlineLookup::Unknown
}

/// Editline function names we recognise and can back with a real
/// [`EmacsFn`]. Order is informational (grouped by concern); lookup
/// is linear, so adding entries is cheap.
const MAPPING_TABLE: &[(&[u8], EmacsFn)] = &[
    // History navigation & search.
    (b"ed-search-prev-history", EmacsFn::HistorySearchBackward),
    (b"ed-search-next-history", EmacsFn::HistorySearchForward),
    (b"ed-prev-history", EmacsFn::PreviousHistory),
    (b"ed-next-history", EmacsFn::NextHistory),
    (b"em-inc-search-prev", EmacsFn::ReverseSearchHistory),
    (b"em-inc-search-next", EmacsFn::ForwardSearchHistory),
    // Cursor motion.
    (b"ed-prev-char", EmacsFn::BackwardChar),
    (b"ed-next-char", EmacsFn::ForwardChar),
    (b"em-next-word", EmacsFn::ForwardWord),
    (b"ed-next-word", EmacsFn::ForwardWord),
    (b"ed-prev-word", EmacsFn::BackwardWord),
    (b"ed-move-to-beg", EmacsFn::BeginningOfLine),
    (b"ed-move-to-end", EmacsFn::EndOfLine),
    // Deletion & killing.
    (b"ed-delete-prev-char", EmacsFn::BackwardDeleteChar),
    (b"ed-delete-next-char", EmacsFn::DeleteChar),
    (b"ed-delete-prev-word", EmacsFn::BackwardKillWord),
    (b"em-delete-prev-word", EmacsFn::BackwardKillWord),
    (b"em-kill-line", EmacsFn::UnixLineDiscard),
    (b"ed-kill-line", EmacsFn::KillLine),
    (b"em-yank", EmacsFn::Yank),
    // Transpose & case.
    (b"ed-transpose-chars", EmacsFn::TransposeChars),
    (b"em-upper-case", EmacsFn::UpcaseWord),
    (b"em-lower-case", EmacsFn::DowncaseWord),
    (b"em-capitol-case", EmacsFn::CapitalizeWord),
    // Misc.
    (b"ed-quoted-insert", EmacsFn::QuotedInsert),
    (b"ed-clear-screen", EmacsFn::ClearScreen),
    (b"ed-newline", EmacsFn::AcceptLine),
    (b"ed-insert", EmacsFn::SelfInsert),
    (b"em-undo", EmacsFn::Undo),
];

/// Editline function names we know about but deliberately don't
/// implement (non-goals in `docs/features/emacs-editing-mode.md`
/// § 15 or simply not-applicable in meiksh's signal handling model).
const UNSUPPORTED_NAMES: &[&[u8]] = &[
    // Mode switches (meiksh enforces mode-at-start-of-line, § 15.7).
    b"vi-cmd-mode",
    b"vi-insert",
    // Mark / region (non-goal § 15.3).
    b"em-set-mark",
    b"em-exchange-mark",
    b"em-kill-region",
    b"em-copy-region",
    b"em-copy-prev-word",
    // Non-goal editing modes / prefix args / meta handling.
    b"em-toggle-overwrite",
    b"em-universal-argument",
    b"em-argument-digit",
    b"em-meta-next",
    // No bindable redraw functions.
    b"ed-redisplay",
    b"ed-refresh",
    b"ed-start-over",
    // Completion list / delete-or-list non-goal § 15.8.
    b"ed-list-choices",
    b"em-delete-or-list",
    // No standalone end-of-file bindable function (behaviour lives
    // on delete-char-on-empty, § 11).
    b"ed-end-of-file",
];

/// Editline's tty-signal passthroughs (`ed-tty-sigint`,
/// `ed-tty-dsusp`, etc.) are handled by meiksh via ordinary signal
/// dispatch rather than bindable functions, so every name in the
/// `ed-tty-*` family classifies as Unsupported.
fn is_ed_tty_family(name: &[u8]) -> bool {
    name.starts_with(b"ed-tty-")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::test_support::assert_no_syscalls;

    #[test]
    fn decode_keyseq_control_notation() {
        assert_no_syscalls(|| {
            assert_eq!(decode_editline_keyseq(b"^A").unwrap(), vec![0x01]);
            assert_eq!(decode_editline_keyseq(b"^a").unwrap(), vec![0x01]);
            assert_eq!(decode_editline_keyseq(b"^[").unwrap(), vec![0x1b]);
            assert_eq!(decode_editline_keyseq(b"^?").unwrap(), vec![0x7f]);
            assert_eq!(
                decode_editline_keyseq(b"^[[A").unwrap(),
                vec![0x1b, b'[', b'A']
            );
            assert_eq!(
                decode_editline_keyseq(b"^[[5~").unwrap(),
                vec![0x1b, b'[', b'5', b'~']
            );
        });
    }

    #[test]
    fn decode_keyseq_backslash_escapes() {
        assert_no_syscalls(|| {
            assert_eq!(
                decode_editline_keyseq(b"\\e[A").unwrap(),
                vec![0x1b, b'[', b'A']
            );
            assert_eq!(
                decode_editline_keyseq(b"\\E[A").unwrap(),
                vec![0x1b, b'[', b'A']
            );
            assert_eq!(decode_editline_keyseq(b"\\n").unwrap(), vec![0x0a]);
            assert_eq!(decode_editline_keyseq(b"\\xff").unwrap(), vec![0xff]);
            assert_eq!(decode_editline_keyseq(b"\\033").unwrap(), vec![0x1b]);
        });
    }

    #[test]
    fn decode_keyseq_mixed_control_and_backslash() {
        assert_no_syscalls(|| {
            // The rxvt-`.shrc` idiom `\e[1;5C` must decode identically
            // whether the user wrote `^[[1;5C` or `\e[1;5C`.
            let caret = decode_editline_keyseq(b"^[[1;5C").unwrap();
            let backslash = decode_editline_keyseq(b"\\e[1;5C").unwrap();
            let upper = decode_editline_keyseq(b"\\E[1;5C").unwrap();
            assert_eq!(caret, vec![0x1b, b'[', b'1', b';', b'5', b'C']);
            assert_eq!(backslash, caret);
            assert_eq!(upper, caret);
        });
    }

    #[test]
    fn decode_keyseq_dangling_caret_errors() {
        assert_no_syscalls(|| {
            assert!(decode_editline_keyseq(b"^").is_err());
        });
    }

    #[test]
    fn decode_keyseq_dangling_backslash_errors() {
        assert_no_syscalls(|| {
            assert!(decode_editline_keyseq(b"\\").is_err());
        });
    }

    #[test]
    fn decode_keyseq_rejects_empty() {
        assert_no_syscalls(|| {
            assert!(decode_editline_keyseq(b"").is_err());
        });
    }

    #[test]
    fn decode_keyseq_literal_bytes_pass_through() {
        assert_no_syscalls(|| {
            // Plain ASCII bytes become themselves; no surprise escaping.
            assert_eq!(decode_editline_keyseq(b"abc").unwrap(), b"abc".to_vec());
        });
    }

    #[test]
    fn translate_every_mapped_name() {
        assert_no_syscalls(|| {
            for (name, expected) in MAPPING_TABLE {
                match translate_editline_function(name) {
                    EditlineLookup::Mapped(got) => assert_eq!(
                        got,
                        *expected,
                        "wrong mapping for {}",
                        String::from_utf8_lossy(name)
                    ),
                    other => panic!(
                        "expected Mapped for {}, got {:?}",
                        String::from_utf8_lossy(name),
                        other
                    ),
                }
            }
        });
    }

    #[test]
    fn translate_readline_native_name_accepted() {
        assert_no_syscalls(|| {
            assert_eq!(
                translate_editline_function(b"history-search-backward"),
                EditlineLookup::Mapped(EmacsFn::HistorySearchBackward)
            );
            assert_eq!(
                translate_editline_function(b"accept-line"),
                EditlineLookup::Mapped(EmacsFn::AcceptLine)
            );
        });
    }

    #[test]
    fn translate_unsupported_family() {
        assert_no_syscalls(|| {
            assert_eq!(
                translate_editline_function(b"vi-cmd-mode"),
                EditlineLookup::Unsupported
            );
            assert_eq!(
                translate_editline_function(b"em-set-mark"),
                EditlineLookup::Unsupported
            );
            assert_eq!(
                translate_editline_function(b"ed-tty-sigint"),
                EditlineLookup::Unsupported
            );
            assert_eq!(
                translate_editline_function(b"ed-tty-dsusp"),
                EditlineLookup::Unsupported
            );
        });
    }

    #[test]
    fn translate_unknown_name_is_distinct_from_unsupported() {
        assert_no_syscalls(|| {
            assert_eq!(
                translate_editline_function(b"totally-made-up"),
                EditlineLookup::Unknown
            );
            // Close typo of a known editline name still classifies as
            // Unknown so the user sees "unknown function" rather than
            // "unsupported editline function".
            assert_eq!(
                translate_editline_function(b"edd-search-prev-history"),
                EditlineLookup::Unknown
            );
        });
    }
}
