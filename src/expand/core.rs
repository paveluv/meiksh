use std::borrow::Cow;
use std::rc::Rc;

use crate::shell::vars::CachedVarBinding;
use crate::syntax::ast::Program;

use super::scratch::ExpandScratch;

#[derive(Debug)]
pub(crate) struct ExpandError {
    pub(crate) message: Box<[u8]>,
}

pub(crate) trait Context {
    fn env_var(&self, name: &[u8]) -> Option<Cow<'_, [u8]>>;

    /// Cached variant of [`Context::env_var`] for AST nodes that
    /// reference a static, parse-time-known name. Implementations
    /// that maintain a slot-indexed variable table (e.g. the real
    /// [`Shell`](crate::shell::state::Shell)) may use `cache` to
    /// skip the name hash on every call after the first. The default
    /// implementation ignores the cache and falls back to
    /// [`Context::env_var`].
    #[inline]
    fn env_var_cached(&self, cache: &CachedVarBinding, name: &[u8]) -> Option<Cow<'_, [u8]>> {
        let _ = cache;
        self.env_var(name)
    }
    fn special_param(&self, name: u8) -> Option<Cow<'_, [u8]>>;
    /// Integer-valued fast path for the numeric special parameters
    /// (`$?`, `$$`, `$!`, `$#`). When `Some(v)` is returned, the caller
    /// must format `v` via a stack buffer and skip the allocating
    /// [`special_param`] path. `None` either means "not an integer
    /// special" (fall through to [`special_param`]) or "the integer
    /// parameter is currently unset" -- in both cases the slow path
    /// below is the correct answer, so the distinction does not matter
    /// at the call site.
    #[inline]
    fn special_param_int(&self, _name: u8) -> Option<i64> {
        None
    }
    fn positional_param(&self, index: usize) -> Option<Cow<'_, [u8]>>;
    fn positional_params(&self) -> &[Vec<u8>];
    fn set_var(&mut self, name: &[u8], value: &[u8]) -> Result<(), ExpandError>;
    fn nounset_enabled(&self) -> bool;
    fn pathname_expansion_enabled(&self) -> bool;
    fn shell_name(&self) -> &[u8];
    fn command_substitute(&mut self, program: &Rc<Program>) -> Result<Vec<u8>, ExpandError>;
    /// Test-only convenience: parse `command` and delegate to
    /// `command_substitute`. Production expansion must never call into the
    /// parser; this lives on the `Context` trait solely so tests can
    /// substitute raw byte slices without wiring up a full parse themselves.
    #[cfg(test)]
    fn command_substitute_raw(&mut self, command: &[u8]) -> Result<Vec<u8>, ExpandError> {
        let program = crate::syntax::parse(command).unwrap_or_default();
        self.command_substitute(&Rc::new(program))
    }
    fn home_dir_for_user(&self, name: &[u8]) -> Option<Cow<'_, [u8]>>;
    fn set_lineno(&mut self, line: usize);
    fn inc_lineno(&mut self);
    fn lineno(&self) -> usize;
    /// Borrow the shared `ExpandScratch` slot for this context. The
    /// slot is the top-of-pool `Option<ExpandScratch>`: callers (notably
    /// [`crate::expand::word::with_scratch`]) `.take()` the scratch to
    /// obtain ownership without contending with other `&mut self` calls
    /// into the context, run their body, then put the (possibly grown)
    /// scratch back. A `None` seen at the slot means the scratch is
    /// currently checked out by an outer expansion frame — nested
    /// callers fall back to constructing a fresh [`ExpandScratch`] so
    /// they never panic on re-entry.
    ///
    /// Wrapping the scratch in `Option` skips the `Default::default()`
    /// placeholder + `drop_in_place` that the older `std::mem::take`
    /// discipline paid on every word expansion.
    fn expand_scratch_slot_mut(&mut self) -> &mut Option<ExpandScratch>;
    /// Borrow the shared [`BytesPool`](crate::exec::scratch::BytesPool)
    /// that the hot literal fast path pulls argv buffers from, and
    /// that `execute_simple` recycles argv / assignment buffers back
    /// into. See `expand_word_into` for the intended use pattern.
    fn bytes_pool_mut(&mut self) -> &mut crate::exec::scratch::BytesPool;
}
