//! Shell variable storage with a stable-slot index model.
//!
//! Design
//! ------
//!
//! Every distinct variable name encountered by the shell is assigned a
//! monotonically increasing `u32` slot. The mapping is kept in
//! [`VarTable::names`] (a `ShellMap<Vec<u8>, u32>`). Entries live in a
//! dense `Vec<Option<EnvEntry>>` ([`VarTable::slots`]); `None` means the
//! variable is currently unset (either never set or cleared via
//! `unset`). Slots are never recycled, so a slot index remains stable
//! for the lifetime of the owning `Shell`.
//!
//! Flags (`exported`, `readonly`) live directly on [`EnvEntry`], which
//! replaces the three separate structures (`env`, `exported`,
//! `readonly`) the shell previously maintained. A single hash
//! operation per mutation is enough to locate both the value and the
//! flags.
//!
//! AST-level memoization
//! --------------------
//!
//! Nodes that reference a static, parse-time-known variable name
//! (simple `$NAME`, plain `${NAME}`, `NAME=value` assignment LHS) embed
//! a [`CachedVarBinding`] slot. The first execution resolves the name
//! to a slot via [`VarTable::ensure_slot`] and stores `slot + 1` in the
//! cell (0 is reserved for "uncached"); subsequent executions read the
//! cell directly, skipping the hash lookup entirely.
//!
//! The cache is a plain `Cell<u32>` — no `Rc`, no `RefCell`, no
//! allocation. Fork-based subshells inherit the AST via OS copy-on-
//! write and evolve their caches independently; divergent slot numbers
//! are harmless because each process also evolves its own
//! [`VarTable`] independently in its private address space.

use std::cell::Cell;

use crate::hash::ShellMap;

/// A single variable's value plus its shell-visible flags. `value`
/// is `None` when the variable has only been decorated with flags
/// (e.g. `readonly X` or `export X` without a value) but never
/// assigned a concrete string. This distinguishes the legacy
/// "exported but unset" state from "exported and set to empty", both
/// of which are observable from shell scripts.
#[derive(Clone, Debug, Default)]
pub(crate) struct EnvEntry {
    pub(crate) value: Option<Vec<u8>>,
    pub(crate) exported: bool,
    pub(crate) readonly: bool,
}

impl EnvEntry {
    pub(crate) fn new(value: Vec<u8>) -> Self {
        Self {
            value: Some(value),
            exported: false,
            readonly: false,
        }
    }
}

/// Shell variable table. Stores every name ever seen by this shell in
/// a dense slot vector so AST nodes can cache a stable `u32` slot
/// index after the first lookup.
#[derive(Clone, Debug, Default)]
pub(crate) struct VarTable {
    /// Slot storage. `None` means the variable is currently unset.
    pub(crate) slots: Vec<Option<EnvEntry>>,
    /// Name → slot-index mapping. Once a name is assigned a slot, the
    /// mapping is never removed (even if the variable is unset).
    pub(crate) names: ShellMap<Vec<u8>, u32>,
}

impl VarTable {
    /// Look up a variable by name. Returns `None` if the name has
    /// never been set, or is currently unset.
    #[inline]
    pub(crate) fn lookup(&self, name: &[u8]) -> Option<&EnvEntry> {
        let slot = *self.names.get(name)?;
        self.slots[slot as usize].as_ref()
    }

    /// Return the (possibly unset) entry at `slot`, or `None` if the
    /// slot is out of bounds for this shell (can happen in a fork that
    /// inherited an AST-level slot cache populated by a post-fork
    /// sibling).
    #[inline]
    pub(crate) fn get_slot(&self, slot: u32) -> Option<&EnvEntry> {
        self.slots.get(slot as usize).and_then(|o| o.as_ref())
    }

    /// Ensure a slot exists for `name`. Creates a fresh `None` slot if
    /// the name is new. Returns the slot index.
    #[inline]
    pub(crate) fn ensure_slot(&mut self, name: &[u8]) -> u32 {
        if let Some(&slot) = self.names.get(name) {
            return slot;
        }
        let slot = self.slots.len() as u32;
        self.slots.push(None);
        self.names.insert(name.to_vec(), slot);
        slot
    }

    /// Look up an existing slot for `name` without allocating a new
    /// one. Returns `None` if the name has never been seen.
    #[inline]
    pub(crate) fn slot_of(&self, name: &[u8]) -> Option<u32> {
        self.names.get(name).copied()
    }

    /// Iterate over every currently-set variable as `(name, entry)`.
    pub(crate) fn iter(&self) -> impl Iterator<Item = (&[u8], &EnvEntry)> {
        self.names.iter().filter_map(|(name, &slot)| {
            self.slots[slot as usize]
                .as_ref()
                .map(|e| (name.as_slice(), e))
        })
    }

    /// Iterate over the names of variables marked `exported`.
    pub(crate) fn iter_exported(&self) -> impl Iterator<Item = (&[u8], &EnvEntry)> {
        self.iter().filter(|(_, e)| e.exported)
    }

    /// Iterate over the names of variables marked `readonly`.
    pub(crate) fn iter_readonly(&self) -> impl Iterator<Item = (&[u8], &EnvEntry)> {
        self.iter().filter(|(_, e)| e.readonly)
    }

    /// True iff `name` is currently a set variable.
    #[inline]
    pub(crate) fn contains_name(&self, name: &[u8]) -> bool {
        self.lookup(name).is_some()
    }

    /// True iff `name` is currently set and exported.
    #[inline]
    pub(crate) fn is_exported(&self, name: &[u8]) -> bool {
        self.lookup(name).is_some_and(|e| e.exported)
    }

    /// True iff `name` is currently marked readonly (whether set or
    /// unset).
    #[inline]
    pub(crate) fn is_readonly(&self, name: &[u8]) -> bool {
        self.lookup(name).is_some_and(|e| e.readonly)
    }

    /// Number of distinct names ever observed by this shell (including
    /// unset ones).
    #[cfg(test)]
    pub(crate) fn slots_len(&self) -> usize {
        self.slots.len()
    }
}

/// Lazily-filled per-AST-node cache of the `VarTable` slot a variable
/// name resolves to. Stored as `slot + 1` so that `0` can represent
/// "uncached" in a single word with niche-free access.
///
/// Operations are interior-mutable via [`Cell`] so cache fills do not
/// require `&mut` on the AST.
#[derive(Default)]
pub(crate) struct CachedVarBinding {
    slot_plus_one: Cell<u32>,
}

impl CachedVarBinding {
    /// Return the cached slot, or `None` if not yet cached.
    #[inline]
    pub(crate) fn get(&self) -> Option<u32> {
        let v = self.slot_plus_one.get();
        if v == 0 { None } else { Some(v - 1) }
    }

    /// Record the resolved slot. Subsequent calls overwrite the slot.
    #[inline]
    pub(crate) fn set(&self, slot: u32) {
        self.slot_plus_one.set(slot.wrapping_add(1));
    }

    /// Resolve the slot for `name`, caching the result on first call.
    #[inline]
    pub(crate) fn resolve(&self, table: &mut VarTable, name: &[u8]) -> u32 {
        match self.get() {
            Some(slot) => slot,
            None => {
                let slot = table.ensure_slot(name);
                self.set(slot);
                slot
            }
        }
    }

    /// Read-only resolve: returns the cached slot if available, else
    /// falls back to a name-based lookup without populating the cache.
    /// Used on the read side to avoid requiring `&mut VarTable` for a
    /// pure lookup.
    #[inline]
    pub(crate) fn resolve_read<'t>(
        &self,
        table: &'t VarTable,
        name: &[u8],
    ) -> Option<&'t EnvEntry> {
        if let Some(slot) = self.get()
            && let entry @ Some(_) = table.get_slot(slot)
        {
            return entry;
        }
        if let Some(slot) = table.slot_of(name) {
            self.set(slot);
            return table.get_slot(slot);
        }
        None
    }
}

impl Clone for CachedVarBinding {
    fn clone(&self) -> Self {
        Self {
            slot_plus_one: Cell::new(self.slot_plus_one.get()),
        }
    }
}

impl std::fmt::Debug for CachedVarBinding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CachedVarBinding").finish_non_exhaustive()
    }
}

impl PartialEq for CachedVarBinding {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}
impl Eq for CachedVarBinding {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_slot_assigns_stable_indices() {
        let mut t = VarTable::default();
        let a = t.ensure_slot(b"FOO");
        let b = t.ensure_slot(b"BAR");
        assert_eq!(a, 0);
        assert_eq!(b, 1);
        // Idempotent.
        assert_eq!(t.ensure_slot(b"FOO"), 0);
        assert_eq!(t.slots_len(), 2);
    }

    #[test]
    fn lookup_returns_none_when_unset() {
        let mut t = VarTable::default();
        let slot = t.ensure_slot(b"FOO");
        assert!(t.lookup(b"FOO").is_none());
        t.slots[slot as usize] = Some(EnvEntry::new(b"v".to_vec()));
        assert_eq!(
            t.lookup(b"FOO").unwrap().value.as_deref(),
            Some(b"v".as_slice())
        );
        t.slots[slot as usize] = None;
        assert!(t.lookup(b"FOO").is_none());
        // Slot mapping survives unset.
        assert_eq!(t.slot_of(b"FOO"), Some(0));
    }

    #[test]
    fn cached_binding_resolves_once() {
        let mut t = VarTable::default();
        let binding = CachedVarBinding::default();
        assert_eq!(binding.get(), None);
        let s1 = binding.resolve(&mut t, b"FOO");
        assert_eq!(s1, 0);
        assert_eq!(binding.get(), Some(0));
        // Second call hits the cache; no new slot.
        let s2 = binding.resolve(&mut t, b"FOO");
        assert_eq!(s2, 0);
        assert_eq!(t.slots_len(), 1);
    }

    #[test]
    fn cached_binding_resolve_read_does_not_allocate_slot() {
        let t = VarTable::default();
        let binding = CachedVarBinding::default();
        assert!(binding.resolve_read(&t, b"NEVER_SET").is_none());
        assert_eq!(binding.get(), None);
    }

    #[test]
    fn iter_skips_unset() {
        let mut t = VarTable::default();
        let a = t.ensure_slot(b"FOO");
        let b = t.ensure_slot(b"BAR");
        t.slots[a as usize] = Some(EnvEntry::new(b"1".to_vec()));
        t.slots[b as usize] = Some(EnvEntry::new(b"2".to_vec()));
        // Unset BAR.
        t.slots[b as usize] = None;
        let names: Vec<_> = t.iter().map(|(n, _)| n.to_vec()).collect();
        assert_eq!(names, vec![b"FOO".to_vec()]);
    }

    #[test]
    fn clone_copies_cached_slot_value() {
        let a = CachedVarBinding::default();
        a.set(7);
        let b = a.clone();
        assert_eq!(b.get(), Some(7));
        // Mutating the clone does not affect the original.
        b.set(9);
        assert_eq!(a.get(), Some(7));
        assert_eq!(b.get(), Some(9));
    }

    #[test]
    fn cached_binding_equality_ignores_cache() {
        let a = CachedVarBinding::default();
        let b = CachedVarBinding::default();
        b.set(3);
        assert_eq!(a, b);
    }
}
