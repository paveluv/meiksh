//! Shared scratch buffers threaded through the expansion pipeline.
//!
//! `ExpandScratch` is owned by `Shell` (and by the test Contexts under
//! `#[cfg(test)]`) and is re-entered on every word expansion via
//! [`Context::expand_scratch_mut`]. All fields are *derived* state - they
//! are logically cleared at the start of each use and exist solely to
//! avoid reallocating a short-lived `Vec` on every expansion.
//!
//! On subshell forks (`Shell: Clone`) the scratch is reset to empty; the
//! buffers are not semantic state and there is no advantage to cloning
//! the cached capacities across a fork boundary.

use super::expand_parts::{ExpandOutput, IfsChar};
use super::model::Segment;

#[derive(Clone, Debug, Default)]
pub(crate) struct ExpandScratch {
    /// Primary `ExpandOutput` reused across every word in a command.
    pub(crate) output: ExpandOutput,
    /// Secondary `ExpandOutput` for sub-expansions (e.g. the inner word of
    /// `${a:-...}` or an arithmetic sub-expression). Re-entrant calls that
    /// find it non-empty allocate their own fresh `ExpandOutput` — see the
    /// take/restore discipline in [`crate::expand::word::with_scratch`].
    pub(crate) output_nested: ExpandOutput,
    /// Cached IFS bytes. Valid iff `ifs_valid` is true.
    pub(crate) ifs_bytes: Vec<u8>,
    /// Cached IFS decomposed into per-character entries. Populated by
    /// `ensure_ifs_cached` alongside `ifs_bytes`; cleared by
    /// [`Self::invalidate_ifs`].
    pub(crate) ifs_chars: Vec<IfsChar>,
    pub(crate) ifs_valid: bool,
    /// Reusable buffer for `char_boundary_offsets`. Callers take it out,
    /// fill, use, then restore.
    pub(crate) char_offsets: Vec<usize>,
    /// Reusable buffer for pattern `Segment` lists built by
    /// `build_pattern_segments`.
    pub(crate) pattern_segments: Vec<Segment>,
    /// Reusable byte buffer for the pre-expanded text of `$((…))`
    /// arithmetic bodies. Callers take it out, fill, use, then restore.
    pub(crate) arith_expr: Vec<u8>,
}

impl ExpandScratch {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Mark the cached IFS bytes stale. Called from `set_var` / `unset_var`
    /// whenever the `IFS` variable is mutated.
    pub(crate) fn invalidate_ifs(&mut self) {
        self.ifs_valid = false;
        self.ifs_bytes.clear();
        self.ifs_chars.clear();
    }
}
