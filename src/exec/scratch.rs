use super::process::ExpandedRedirection;

/// Free-list of cleared `Vec<u8>` buffers. Used to recycle argv /
/// assignment-value / redirection-target buffers around
/// `execute_simple` so the hot literal-expansion path in
/// `expand::word` can pull a pre-sized buffer rather than calling
/// `Vec::with_capacity` or `to_vec` on every word.
///
/// Size-bounded so long-running scripts don't pin unbounded memory.
#[derive(Default, Debug)]
pub(crate) struct BytesPool {
    bufs: Vec<Vec<u8>>,
}

impl BytesPool {
    const MAX_POOLED: usize = 64;

    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Pop a cleared buffer (with preserved capacity) or return a
    /// fresh `Vec::new()` if the pool is empty.
    #[inline]
    pub(crate) fn take(&mut self) -> Vec<u8> {
        self.bufs.pop().unwrap_or_default()
    }

    /// Return a buffer to the pool. Cleared before storage so callers
    /// that accidentally read from a recycled buffer observe an empty
    /// slice rather than stale bytes. Dropped if the pool is full.
    #[inline]
    pub(crate) fn recycle(&mut self, mut v: Vec<u8>) {
        v.clear();
        if self.bufs.len() < Self::MAX_POOLED {
            self.bufs.push(v);
        }
    }
}

/// Cloning a `BytesPool` is cheap (returns an empty pool). Same
/// rationale as `ExecScratchPool::clone` - pooled state is capacity
/// and should not cross a `Shell::clone` boundary.
impl Clone for BytesPool {
    fn clone(&self) -> Self {
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_pool_take_and_recycle_reuses_buffer() {
        let mut pool = BytesPool::new();
        let mut buf = pool.take();
        assert!(buf.is_empty());
        buf.extend_from_slice(b"hello world");
        let cap_before = buf.capacity();
        pool.recycle(buf);
        let buf2 = pool.take();
        assert!(buf2.is_empty());
        assert!(buf2.capacity() >= cap_before);
    }

    #[test]
    fn bytes_pool_respects_cap() {
        let mut pool = BytesPool::new();
        for _ in 0..BytesPool::MAX_POOLED + 16 {
            pool.recycle(Vec::with_capacity(8));
        }
        assert_eq!(pool.bufs.len(), BytesPool::MAX_POOLED);
    }

    #[test]
    fn bytes_pool_recycle_clears_content() {
        let mut pool = BytesPool::new();
        let mut buf = Vec::new();
        buf.extend_from_slice(b"secret");
        pool.recycle(buf);
        let buf2 = pool.take();
        assert!(buf2.is_empty(), "recycled buffer must be cleared");
    }

    #[test]
    fn exec_scratch_clear_into_pool_recycles_buffers() {
        let mut pool = BytesPool::new();
        let mut scratch = ExecScratch::default();
        scratch.argv.push(b"hello".to_vec());
        scratch.argv.push(b"world".to_vec());
        scratch.assignments.push((b"FOO".to_vec(), b"bar".to_vec()));
        scratch.clear_into_pool(&mut pool);
        assert!(scratch.argv.is_empty());
        assert!(scratch.assignments.is_empty());
        // argv: 2 bufs; assignments: 2 bufs (name + value).
        assert_eq!(pool.bufs.len(), 4);
    }

    #[test]
    fn exec_scratch_pool_take_and_push_reuses_outer_capacity() {
        let mut pool = ExecScratchPool::new();
        let mut s = pool.take();
        s.argv.reserve(16);
        let cap = s.argv.capacity();
        s.clear();
        pool.push_cleared(s);
        let s2 = pool.take();
        assert!(s2.argv.capacity() >= cap);
    }
}

/// Per-`execute_simple` scratch buffers used to build the expanded
/// argv / assignments / redirections without freshly allocating the
/// outer `Vec`s on every call. The buffers are owned by the
/// [`ExecScratchPool`] on `Shell` and are checked out / recycled
/// around each `execute_simple` invocation.
///
/// Only the *outer* `Vec` capacity is reused in phase 5. The inner
/// `Vec<u8>` elements are still allocated fresh per call; the inner
/// free-list lives in phase 5b (`BytesPool`).
#[derive(Default, Debug)]
pub(crate) struct ExecScratch {
    pub(super) argv: Vec<Vec<u8>>,
    pub(super) assignments: Vec<(Vec<u8>, Vec<u8>)>,
    pub(super) redirections: Vec<ExpandedRedirection>,
}

impl ExecScratch {
    pub(super) fn clear(&mut self) {
        self.argv.clear();
        self.assignments.clear();
        self.redirections.clear();
    }

    /// Clear, returning the inner `Vec<u8>` buffers to `bytes_pool`
    /// so they can be reused by the next expansion rather than
    /// dropped. The outer `Vec` capacities are still preserved on
    /// `self`.
    pub(crate) fn clear_into_pool(&mut self, bytes_pool: &mut BytesPool) {
        for v in self.argv.drain(..) {
            bytes_pool.recycle(v);
        }
        for (name, value) in self.assignments.drain(..) {
            bytes_pool.recycle(name);
            bytes_pool.recycle(value);
        }
        for redir in self.redirections.drain(..) {
            bytes_pool.recycle(redir.target);
            if let Some(body) = redir.here_doc_body {
                bytes_pool.recycle(body);
            }
        }
    }
}

/// Free-list of `ExecScratch` buffers backing `execute_simple`.
///
/// Each `execute_simple` call pops a scratch via [`Self::take`]; on
/// return it pushes the (cleared) scratch back via [`Self::recycle`].
/// Re-entrant calls (command substitutions, function bodies,
/// pipelines) simply pop a separate scratch - if the pool is empty we
/// allocate a fresh one, so correctness never depends on pool state.
///
/// The pool is bounded to a small number of entries so long-running
/// shells don't hold unbounded memory.
#[derive(Default, Debug)]
pub(crate) struct ExecScratchPool {
    free: Vec<ExecScratch>,
}

/// Clone returns an empty pool: scratch buffers are purely a
/// capacity-reuse optimization and are cheap to re-grow. Cloning the
/// `Shell` (e.g. to snapshot for a subshell) should not carry
/// accidentally shared buffer capacity.
impl Clone for ExecScratchPool {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl ExecScratchPool {
    const MAX_POOLED: usize = 8;

    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Pop a scratch from the free-list, or create a fresh one.
    pub(crate) fn take(&mut self) -> ExecScratch {
        self.free.pop().unwrap_or_default()
    }

    /// Store an already-cleared scratch back onto the free-list, up
    /// to `MAX_POOLED` entries. Callers are responsible for clearing
    /// the scratch first (typically via `ExecScratch::clear_into_pool`
    /// so inner buffers are recycled rather than dropped).
    pub(crate) fn push_cleared(&mut self, s: ExecScratch) {
        debug_assert!(
            s.argv.is_empty() && s.assignments.is_empty() && s.redirections.is_empty(),
            "ExecScratchPool::push_cleared expected a cleared scratch",
        );
        if self.free.len() < Self::MAX_POOLED {
            self.free.push(s);
        }
    }
}
