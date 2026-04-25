use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::rc::Rc;

use crate::exec::scratch::{BytesPool, ExecScratch, ExecScratchPool};
use crate::expand::scratch::ExpandScratch;
use crate::hash::ShellMap;
use crate::sys;

use super::jobs::Job;
use super::options::ShellOptions;
use super::traps::{TrapAction, TrapCondition};
use super::vars::{EnvEntry, VarTable};

pub(crate) enum FlowSignal {
    Continue(i32),
    UtilityError(i32),
    Exit(i32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PendingControl {
    Return(i32),
    Break(usize),
    Continue(usize),
}

/// A stable anchor for a shell-function body. Holders of
/// `Rc<FunctionSlot>` observe live redefinitions via `slot.body.borrow()`
/// and see `None` after `unset -f`. The indirection lets the per-
/// `SimpleCommand` `argv[0]` memo cache an `Rc<FunctionSlot>` whose
/// identity survives map rehashes and redefinitions.
#[derive(Debug)]
pub(crate) struct FunctionSlot {
    pub(crate) body: RefCell<Option<Rc<crate::syntax::ast::Command>>>,
}

impl FunctionSlot {
    pub(crate) fn new(body: Rc<crate::syntax::ast::Command>) -> Self {
        Self {
            body: RefCell::new(Some(body)),
        }
    }
}

#[derive(Clone)]
pub(crate) struct SharedEnv {
    /// Variable storage. Replaces the three legacy structures
    /// (separate env map + exported / readonly `BTreeSet`s); every
    /// variable's value and flags live together in a single
    /// [`EnvEntry`] looked up by dense slot index.
    pub(crate) vars: VarTable,
    pub(crate) aliases: ShellMap<Box<[u8]>, Box<[u8]>>,
    pub(crate) functions: ShellMap<Vec<u8>, Rc<FunctionSlot>>,
    pub(crate) path_cache: ShellMap<Box<[u8]>, Vec<u8>>,
    pub(crate) history: Vec<Box<[u8]>>,
    pub(crate) mail_sizes: ShellMap<Box<[u8]>, u64>,
}

#[derive(Clone)]
pub(crate) struct Shell {
    pub(crate) options: ShellOptions,
    pub(crate) shell_name: Box<[u8]>,
    pub(crate) shared: Rc<SharedEnv>,
    pub(crate) positional: Vec<Vec<u8>>,
    pub(crate) last_status: i32,
    pub(crate) last_background: Option<sys::types::Pid>,
    pub(crate) running: bool,
    pub(crate) jobs: Vec<Job>,
    pub(crate) known_pid_statuses: HashMap<sys::types::Pid, i32>,
    pub(crate) known_job_statuses: HashMap<usize, i32>,
    /// Pre-formatted job-status notification messages whose printing
    /// has been deferred to the next prompt. Populated by
    /// [`crate::interactive::notify::stash_or_print_notifications`]
    /// when the editor's blocking read is woken by `SIGCHLD` while
    /// `set -b` (the `notify` option) is *off* — POSIX § 2.11 then
    /// requires the message to be written "before the next prompt".
    /// Drained at the top of the REPL loop (see
    /// [`crate::interactive::repl`]). Always empty in non-interactive
    /// shells.
    pub(crate) pending_notifications: Vec<Vec<u8>>,
    pub(crate) trap_actions: BTreeMap<TrapCondition, TrapAction>,
    pub(crate) ignored_on_entry: BTreeSet<TrapCondition>,
    pub(crate) subshell_saved_traps: Option<BTreeMap<TrapCondition, TrapAction>>,
    pub(crate) loop_depth: usize,
    pub(crate) function_depth: usize,
    /// Nesting depth of dot (`source_path`) files being executed.
    pub(crate) source_depth: usize,
    pub(crate) pending_control: Option<PendingControl>,
    pub(crate) interactive: bool,
    pub(crate) errexit_suppressed: bool,
    pub(crate) owns_terminal: bool,
    pub(crate) in_subshell: bool,
    /// Depth of subshell nesting. `0` in the top-level shell,
    /// incremented by exactly one every time we transition to a
    /// subshell context (command substitution, `(...)`, left-side of
    /// a background or piped command, etc.). Used by the `xtrace`
    /// writer to duplicate the first character of `PS4` once per
    /// level per `docs/features/ps1-prompt-extensions.md` § 3.5.
    pub(crate) subshell_nesting_level: u32,
    pub(crate) wait_was_interrupted: bool,
    pub(crate) pid: sys::types::Pid,
    pub(crate) lineno: usize,
    pub(crate) mail_last_check: u64,
    /// Per-session counter of accepted, non-empty input lines. Starts
    /// at `0`; incremented to `1` just before executing the first
    /// command in the session, `2` before the second, and so on. See
    /// `docs/features/ps1-prompt-extensions.md` § 6.1 (`\#`).
    ///
    /// This counter is session-local (not inherited by subshells) and
    /// is *not* backed by `$HISTCMD`: it has its own lifecycle, it
    /// keeps counting when history is disabled, and it is never
    /// rewound by `history -c`.
    pub(crate) session_command_counter: u64,
    /// Reusable scratch buffers for word expansion. Derived state - reset
    /// via `invalidate_ifs` on `IFS` mutation and (optionally) at subshell
    /// fork boundaries. Clone produces a semantically-equivalent but
    /// capacity-wise empty scratch; buffers will regrow on first use.
    ///
    /// Stored as `Option<ExpandScratch>` so that the `with_scratch`
    /// pool discipline can `.take()` ownership without allocating /
    /// dropping a `Default`-constructed placeholder on every word
    /// expansion. The slot is `Some` at steady state and briefly
    /// `None` while an outer `with_scratch` body is running; nested
    /// re-entrant frames observe `None` and construct a fresh
    /// scratch locally.
    pub(crate) expand_scratch: Option<ExpandScratch>,
    /// Free-list of per-`execute_simple` scratch buffers holding the
    /// expanded argv / assignments / redirections outer `Vec`s.
    /// See [`ExecScratchPool`] for the re-entrancy contract.
    pub(crate) exec_scratch_pool: ExecScratchPool,
    /// Free-list of cleared `Vec<u8>` buffers. Producers include the
    /// literal fast path in word expansion; consumers include the
    /// recycling of argv / assignment / redirection buffers after
    /// `execute_simple`.
    pub(crate) bytes_pool: BytesPool,
}

impl Shell {
    /// Shared variable table (all set/unset/flag state).
    #[inline]
    pub(crate) fn vars(&self) -> &VarTable {
        &self.shared.vars
    }

    /// Mutable variable table. Goes through `Rc::make_mut`, so it is
    /// copy-on-write against any outstanding shared clone (e.g. a
    /// subshell fork that has not diverged yet).
    #[inline]
    pub(crate) fn vars_mut(&mut self) -> &mut VarTable {
        &mut Rc::make_mut(&mut self.shared).vars
    }

    /// Returns the value bytes for `name`, or `None` if the variable
    /// is currently unset (whether or not a flag-only entry exists).
    #[inline]
    pub(crate) fn var_value(&self, name: &[u8]) -> Option<&[u8]> {
        self.vars().lookup(name).and_then(|e| e.value.as_deref())
    }

    /// True iff `name` is currently set and exported.
    #[inline]
    pub(crate) fn is_exported(&self, name: &[u8]) -> bool {
        self.vars().is_exported(name)
    }

    /// True iff `name` is currently marked readonly.
    #[inline]
    pub(crate) fn is_readonly(&self, name: &[u8]) -> bool {
        self.vars().is_readonly(name)
    }

    /// Insert or overwrite `name` with `value`, preserving existing
    /// flags. Test-support hook used by callers that previously poked
    /// `env_mut().insert(...)` directly.
    pub(crate) fn env_set_raw(&mut self, name: Vec<u8>, value: Vec<u8>) {
        let vars = self.vars_mut();
        let slot = vars.ensure_slot(&name) as usize;
        match &mut vars.slots[slot] {
            Some(entry) => entry.value = Some(value),
            None => vars.slots[slot] = Some(EnvEntry::new(value)),
        }
    }

    /// Remove the value bound to `name`, preserving the flag state
    /// (matching the legacy `env_mut().remove` + separate flag-set
    /// semantics). The slot itself is kept in the name table so any
    /// cached AST handle continues to resolve to it.
    pub(crate) fn env_remove_raw(&mut self, name: &[u8]) -> Option<Vec<u8>> {
        let vars = self.vars_mut();
        let slot = vars.slot_of(name)?;
        let entry = vars.slots[slot as usize].take()?;
        entry.value
    }

    /// Mark `name` as exported. If the variable has no entry yet,
    /// a flag-only entry is created (no value assigned) so that
    /// subsequent `export -p` output matches POSIX: `export NAME`
    /// rather than `export NAME=''`.
    pub(crate) fn mark_exported(&mut self, name: &[u8]) {
        let vars = self.vars_mut();
        let slot = vars.ensure_slot(name) as usize;
        match &mut vars.slots[slot] {
            Some(entry) => entry.exported = true,
            None => {
                vars.slots[slot] = Some(EnvEntry {
                    value: None,
                    exported: true,
                    readonly: false,
                });
            }
        }
    }

    /// Remove the exported flag from `name`, if any. No-op if the
    /// variable is not set.
    pub(crate) fn unmark_exported(&mut self, name: &[u8]) {
        let vars = self.vars_mut();
        if let Some(slot) = vars.slot_of(name)
            && let Some(entry) = vars.slots[slot as usize].as_mut()
        {
            entry.exported = false;
        }
    }

    /// Legacy compat accessor exposing a map-like view over the
    /// variable table. Returns a thin adapter with `get`, `insert`,
    /// `remove`, `iter`, and `get_mut`, matching the previous
    /// `ShellMap<Vec<u8>, Vec<u8>>` surface used by a large test
    /// population and a handful of call sites that mutate individual
    /// variable entries without touching flag state. Non-test paths
    /// should prefer the direct `set_var` / `var_value` APIs.
    #[inline]
    pub(crate) fn env(&self) -> EnvView<'_> {
        EnvView { table: self.vars() }
    }

    /// Mutable variant of [`Shell::env`].
    #[inline]
    pub(crate) fn env_mut(&mut self) -> EnvViewMut<'_> {
        EnvViewMut {
            table: self.vars_mut(),
        }
    }

    pub(crate) fn aliases(&self) -> &ShellMap<Box<[u8]>, Box<[u8]>> {
        &self.shared.aliases
    }
    pub(crate) fn aliases_mut(&mut self) -> &mut ShellMap<Box<[u8]>, Box<[u8]>> {
        &mut Rc::make_mut(&mut self.shared).aliases
    }
    pub(crate) fn functions(&self) -> &ShellMap<Vec<u8>, Rc<FunctionSlot>> {
        &self.shared.functions
    }
    pub(crate) fn functions_mut(&mut self) -> &mut ShellMap<Vec<u8>, Rc<FunctionSlot>> {
        &mut Rc::make_mut(&mut self.shared).functions
    }

    /// Look up a function body by name. Returns `None` if no slot exists
    /// or if the slot was cleared by a recent `unset -f`.
    pub(crate) fn lookup_function(&self, name: &[u8]) -> Option<Rc<crate::syntax::ast::Command>> {
        self.shared
            .functions
            .get(name)
            .and_then(|slot| slot.body.borrow().as_ref().map(Rc::clone))
    }

    /// Look up the `FunctionSlot` handle for `name`. Used by the
    /// `argv[0]` memo on `SimpleCommand` to cache a stable handle whose
    /// body can be re-checked directly without re-probing the map.
    pub(crate) fn lookup_function_slot(&self, name: &[u8]) -> Option<Rc<FunctionSlot>> {
        self.shared.functions.get(name).map(Rc::clone)
    }

    /// Define or redefine a function. If a slot already exists for
    /// `name`, mutate its `body` in place so any cached handles observe
    /// the new definition; otherwise create a fresh slot.
    pub(crate) fn define_function(&mut self, name: Vec<u8>, body: Rc<crate::syntax::ast::Command>) {
        let map = self.functions_mut();
        if let Some(slot) = map.get(name.as_slice()) {
            *slot.body.borrow_mut() = Some(body);
        } else {
            map.insert(name, Rc::new(FunctionSlot::new(body)));
        }
    }

    /// Unset a function. Any outstanding `Rc<FunctionSlot>` handles will
    /// observe `body.is_none()` and fall back through the classifier.
    pub(crate) fn unset_function(&mut self, name: &[u8]) {
        let map = self.functions_mut();
        if let Some(slot) = map.get(name) {
            *slot.body.borrow_mut() = None;
        }
        map.remove(name);
    }

    /// Take an [`ExecScratch`] out of the pool, or create a fresh one
    /// if the pool is empty.
    #[inline]
    pub(crate) fn take_exec_scratch(&mut self) -> ExecScratch {
        self.exec_scratch_pool.take()
    }

    /// Return an `ExecScratch` to the pool. Inner `Vec<u8>` buffers
    /// are drained into `bytes_pool` so they can be re-used by the
    /// next word-expansion fast path.
    ///
    /// Fast path: an already-empty scratch (no argv, no assignments,
    /// no redirections) skips the three drain loops and returns the
    /// scratch directly. This is the common case for `_fn_noop`-style
    /// zero-arg function calls in tight loops, where `execute_simple`
    /// has already moved every inner `Vec<u8>` out of the scratch
    /// (into `shell.positional` / the expansion result) before
    /// returning.
    #[inline]
    pub(crate) fn recycle_exec_scratch(&mut self, mut scratch: ExecScratch) {
        if scratch.is_empty() {
            self.exec_scratch_pool.push_cleared(scratch);
            return;
        }
        scratch.clear_into_pool(&mut self.bytes_pool);
        self.exec_scratch_pool.push_cleared(scratch);
    }
    pub(crate) fn path_cache(&self) -> &ShellMap<Box<[u8]>, Vec<u8>> {
        &self.shared.path_cache
    }
    pub(crate) fn path_cache_mut(&mut self) -> &mut ShellMap<Box<[u8]>, Vec<u8>> {
        &mut Rc::make_mut(&mut self.shared).path_cache
    }
    pub(crate) fn history(&self) -> &Vec<Box<[u8]>> {
        &self.shared.history
    }
    pub(crate) fn history_mut(&mut self) -> &mut Vec<Box<[u8]>> {
        &mut Rc::make_mut(&mut self.shared).history
    }
    pub(crate) fn mail_sizes(&self) -> &ShellMap<Box<[u8]>, u64> {
        &self.shared.mail_sizes
    }
    pub(crate) fn mail_sizes_mut(&mut self) -> &mut ShellMap<Box<[u8]>, u64> {
        &mut Rc::make_mut(&mut self.shared).mail_sizes
    }
}

/// Read-only map-like view of the variable table. Preserves the
/// `ShellMap<Vec<u8>, Vec<u8>>` iteration surface (`iter`) that
/// existed before the `VarTable` refactor, so callers that only
/// enumerate set-variable pairs do not need to be rewritten.
pub(crate) struct EnvView<'a> {
    table: &'a VarTable,
}

impl<'a> EnvView<'a> {
    pub(crate) fn iter(&self) -> impl Iterator<Item = (&[u8], &[u8])> {
        self.table
            .iter()
            .filter_map(|(n, e)| e.value.as_deref().map(|v| (n, v)))
    }
}

/// Mutable map-like view of the variable table. Provides the legacy
/// `insert` / `remove` mutators without touching flag bits. Flag
/// changes must go through [`Shell::mark_exported`] /
/// [`Shell::mark_readonly`].
pub(crate) struct EnvViewMut<'a> {
    table: &'a mut VarTable,
}

impl<'a> EnvViewMut<'a> {
    #[inline]
    pub(crate) fn insert(&mut self, name: Vec<u8>, value: Vec<u8>) -> Option<Vec<u8>> {
        let slot = self.table.ensure_slot(&name) as usize;
        let prev = self.table.slots[slot]
            .take()
            .map(|e| (e.value, e.exported, e.readonly));
        match prev {
            Some((old_value, exported, readonly)) => {
                self.table.slots[slot] = Some(EnvEntry {
                    value: Some(value),
                    exported,
                    readonly,
                });
                old_value
            }
            None => {
                self.table.slots[slot] = Some(EnvEntry::new(value));
                None
            }
        }
    }

    #[inline]
    pub(crate) fn remove(&mut self, name: &[u8]) -> Option<Vec<u8>> {
        let slot = self.table.slot_of(name)?;
        self.table.slots[slot as usize].take().and_then(|e| e.value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::shell::test_support::test_shell;
    use crate::trace_entries;

    #[test]
    fn run_builtin_returns_correct_flow_signals() {
        crate::sys::test_support::run_trace(trace_entries![], || {
            let mut shell = test_shell();

            let flow = shell
                .run_builtin(
                    &[b"export".to_vec(), b"FLOW=1".to_vec()],
                    &[(b"ASSIGN".to_vec(), b"2".to_vec())],
                )
                .expect("builtin");
            assert!(matches!(flow, FlowSignal::Continue(0)));
            assert_eq!(shell.get_var(b"ASSIGN"), Some(b"2".as_slice()));
            assert_eq!(shell.get_var(b"FLOW"), Some(b"1".as_slice()));
            assert!(shell.is_exported(b"FLOW"));

            let flow = shell
                .run_builtin(&[b"exit".to_vec(), b"9".to_vec()], &[])
                .expect("exit builtin");
            assert!(matches!(flow, FlowSignal::Exit(9)));

            shell.function_depth = 1;
            let flow = shell
                .run_builtin(&[b"return".to_vec(), b"4".to_vec()], &[])
                .expect("return builtin");
            assert!(matches!(flow, FlowSignal::Continue(4)));
            assert_eq!(shell.pending_control, Some(PendingControl::Return(4)));
            shell.pending_control = None;
            shell.function_depth = 0;

            shell.loop_depth = 2;
            let flow = shell
                .run_builtin(&[b"break".to_vec(), b"5".to_vec()], &[])
                .expect("break builtin");
            assert!(matches!(flow, FlowSignal::Continue(0)));
            assert_eq!(shell.pending_control, Some(PendingControl::Break(2)));
            shell.pending_control = None;
        });
    }

    #[test]
    fn define_function_redefines_existing_slot_in_place() {
        // Defining `f` twice should reuse the same `Rc<FunctionSlot>`,
        // taking the in-place mutation arm at line 271.
        crate::sys::test_support::assert_no_syscalls(|| {
            let mut shell = test_shell();
            let _ = shell.execute_string(b"f() { :; }");
            let slot1 = shell.lookup_function_slot(b"f").expect("first slot");
            let _ = shell.execute_string(b"f() { true; }");
            let slot2 = shell.lookup_function_slot(b"f").expect("second slot");
            assert!(Rc::ptr_eq(&slot1, &slot2));
        });
    }

    #[test]
    fn return_in_dot_sourced_file_exits_source_with_status() {
        crate::sys::test_support::assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.source_depth = 1;
            let status = shell
                .execute_string(b":; return 5; :")
                .expect("return from source");
            assert_eq!(status, 5);
            assert!(shell.pending_control.is_none());
        });
    }
}
