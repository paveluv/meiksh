//! Shared line-editor primitives consumed by both [`vi_editing`] and
//! [`emacs_editing`]. Each submodule exposes a narrow surface so the
//! two editors can share terminal setup, byte I/O, redraw math, word
//! boundaries, history search, and bracketed-paste framing without
//! duplicating logic.
//!
//! `#[allow(dead_code)]` is applied module-wide: some submodules (for
//! example `bracketed_paste`, `history_search`'s forward scan) only
//! light up once `emacs_editing` is wired up in Stage B. The cfg(test)
//! unit tests in each submodule exercise them meanwhile.
#![allow(dead_code)]
//!
//! Items are `pub(crate)` because they are consumed from sibling
//! modules (`crate::interactive::vi_editing`, `::emacs_editing`,
//! eventually `::emacs_editing::functions`, etc.) and nothing outside
//! the interactive stack should reach into them. Nothing is re-exported
//! via `pub use`, per [`docs/IMPLEMENTATION_POLICY.md`].
//!
//! [`vi_editing`]: super::vi_editing
//! [`emacs_editing`]: super::emacs_editing
//! [`docs/IMPLEMENTATION_POLICY.md`]: ../../../docs/IMPLEMENTATION_POLICY.md

pub(crate) mod bracketed_paste;
pub(crate) mod history_search;
pub(crate) mod input;
pub(crate) mod raw_mode;
pub(crate) mod redraw;
pub(crate) mod words;
