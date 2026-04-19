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

use super::expand_parts::ExpandOutput;

#[derive(Clone, Debug, Default)]
pub(crate) struct ExpandScratch {
    /// Primary `ExpandOutput` reused across every word in a command.
    pub(crate) output: ExpandOutput,
    /// Cached IFS bytes. Valid iff `ifs_valid` is true.
    pub(crate) ifs_bytes: Vec<u8>,
    pub(crate) ifs_valid: bool,
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
    }
}
