use std::rc::Rc;

use crate::syntax::ast::Program;
use crate::sys;

use super::error::ShellError;
use super::state::Shell;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum TrapCondition {
    Exit,
    Signal(sys::types::Pid),
}

#[derive(Clone, Debug)]
pub(crate) enum TrapAction {
    Ignore,
    Command {
        text: Box<[u8]>,
        program: Rc<Program>,
    },
}

#[cfg(test)]
impl TrapAction {
    pub(crate) fn command(text: &[u8]) -> Self {
        let program = crate::syntax::parse_with_aliases(text, &crate::hash::ShellMap::default())
            .unwrap_or_else(|_| Program { items: Vec::new() });
        TrapAction::Command {
            text: text.into(),
            program: Rc::new(program),
        }
    }
}

impl Shell {
    pub(crate) fn trap_action(&self, condition: TrapCondition) -> Option<&TrapAction> {
        self.trap_actions.get(&condition)
    }

    pub(crate) fn set_trap(
        &mut self,
        condition: TrapCondition,
        action: Option<TrapAction>,
    ) -> Result<(), ShellError> {
        if !self.interactive && self.ignored_on_entry.contains(&condition) {
            return Ok(());
        }
        self.subshell_saved_traps = None;
        if let TrapCondition::Signal(signal) = condition {
            match action.as_ref() {
                Some(TrapAction::Ignore) => sys::process::ignore_signal(signal)
                    .map_err(|e| self.diagnostic_syserr(1, &e))?,
                Some(TrapAction::Command { .. }) => {
                    sys::process::install_shell_signal_handler(signal)
                        .map_err(|e| self.diagnostic_syserr(1, &e))?
                }
                None => sys::process::default_signal_action(signal)
                    .map_err(|e| self.diagnostic_syserr(1, &e))?,
            }
        }
        match action {
            Some(action) => {
                self.trap_actions.insert(condition, action);
            }
            None => {
                self.trap_actions.remove(&condition);
            }
        }
        Ok(())
    }

    pub(crate) fn reset_traps_for_subshell(&mut self) -> Result<(), ShellError> {
        if self.subshell_saved_traps.is_none() {
            self.subshell_saved_traps = Some(self.trap_actions.clone());
        }
        let to_reset: Vec<TrapCondition> = self
            .trap_actions
            .iter()
            .filter_map(|(cond, action)| match action {
                TrapAction::Command { .. } => Some(*cond),
                TrapAction::Ignore => None,
            })
            .collect();
        for cond in to_reset {
            if let TrapCondition::Signal(signal) = cond {
                sys::process::default_signal_action(signal)
                    .map_err(|e| self.diagnostic_syserr(1, &e))?;
            }
            self.trap_actions.remove(&cond);
        }
        Ok(())
    }

    pub(crate) fn restore_signals_for_child(&self) {
        // A signal is "user-trapped to ignore" only if `trap '' SIG`
        // was executed *after* shell startup. The `Shell::new`
        // constructor seeds `trap_actions` from `ignored_on_entry`,
        // so a `trap_actions[SIG] == Ignore` entry is "user-trapped"
        // iff the signal is NOT in `ignored_on_entry`. We use this
        // distinction to honour POSIX § 2.14 trap semantics
        // (inherited-ignored signals stay ignored across `fork+exec`)
        // while still resetting *only-inherited* job-control signals
        // to the default disposition for foreground children spawned
        // under `set -m` — the practical bash/ksh behaviour that
        // makes `Ctrl-Z`/`SIGTSTP` actually stop the foreground job
        // even when the shell was launched from a parent that had
        // those signals already set to `SIG_IGN` (e.g. an
        // interactive ksh on OpenBSD propagates `SIG_IGN` for
        // SIGTSTP/SIGTTIN/SIGTTOU through every child it spawns,
        // including `cargo` -> `expect_pty` -> `meiksh`).
        let user_trap_ignored = |sig: i32| -> bool {
            let cond = TrapCondition::Signal(sig);
            matches!(self.trap_actions.get(&cond), Some(TrapAction::Ignore))
                && !self.ignored_on_entry.contains(&cond)
        };
        if self.interactive {
            for sig in [sys::constants::SIGTERM, sys::constants::SIGQUIT] {
                if !user_trap_ignored(sig) {
                    let _ = sys::process::default_signal_action(sig);
                }
            }
            if !user_trap_ignored(sys::constants::SIGINT) {
                let _ = sys::process::default_signal_action(sys::constants::SIGINT);
            }
        }
        if self.options.monitor {
            for sig in [
                sys::constants::SIGTSTP,
                sys::constants::SIGTTIN,
                sys::constants::SIGTTOU,
            ] {
                if !user_trap_ignored(sig) {
                    let _ = sys::process::default_signal_action(sig);
                }
            }
        }
    }

    pub(crate) fn run_pending_traps(&mut self) -> Result<(), ShellError> {
        // Fast path: if no traps are installed and no signals are pending,
        // avoid the atomic swap, Vec allocation, and BTreeMap lookup that
        // `take_pending_signals` would otherwise perform on every command.
        if self.trap_actions.is_empty() && sys::process::pending_signal_bits() == 0 {
            return Ok(());
        }
        for signal in sys::process::take_pending_signals() {
            let Some(TrapAction::Command { program, .. }) =
                self.trap_actions.get(&TrapCondition::Signal(signal))
            else {
                continue;
            };
            let program = Rc::clone(program);
            self.execute_trap_action(&program, self.last_status)?;
            if !self.running {
                break;
            }
        }
        Ok(())
    }

    pub(crate) fn run_exit_trap(&mut self, status: i32) -> Result<i32, ShellError> {
        let Some(TrapAction::Command { program, .. }) = self.trap_actions.get(&TrapCondition::Exit)
        else {
            self.last_status = status;
            return Ok(status);
        };
        let program = Rc::clone(program);
        self.execute_trap_action(&program, status)
    }

    pub(super) fn execute_trap_action(
        &mut self,
        program: &Program,
        preserved_status: i32,
    ) -> Result<i32, ShellError> {
        let saved_lineno = self.lineno;
        let was_running = self.running;
        self.running = true;
        self.last_status = preserved_status;
        let status = self.execute_program(program)?;
        self.lineno = saved_lineno;
        if self.running {
            self.running = was_running;
            self.last_status = preserved_status;
            Ok(preserved_status)
        } else {
            self.last_status = status;
            Ok(status)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::sys;
    use crate::sys::test_support::{assert_no_syscalls, run_trace};
    use crate::trace_entries;

    use crate::shell::test_support::test_shell;

    #[test]
    fn set_trap_ignore_and_default_use_signal_syscall() {
        run_trace(
            trace_entries![
                signal(int(sys::constants::SIGTERM as i64), _) -> 0,
                signal(int(sys::constants::SIGTERM as i64), _) -> 0,
            ],
            || {
                let mut shell = test_shell();
                shell
                    .set_trap(
                        TrapCondition::Signal(sys::constants::SIGTERM),
                        Some(TrapAction::Ignore),
                    )
                    .expect("ignore");
                assert!(matches!(
                    shell.trap_action(TrapCondition::Signal(sys::constants::SIGTERM)),
                    Some(TrapAction::Ignore)
                ));
                shell
                    .set_trap(TrapCondition::Signal(sys::constants::SIGTERM), None)
                    .expect("default");
                assert!(
                    shell
                        .trap_action(TrapCondition::Signal(sys::constants::SIGTERM))
                        .is_none()
                );
            },
        );
    }

    #[test]
    fn reset_traps_for_subshell_keeps_ignore_removes_command() {
        run_trace(
            trace_entries![
                signal(int(crate::sys::constants::SIGTERM as i64), _) -> 0,
            ],
            || {
                let mut shell = test_shell();
                shell.trap_actions.insert(
                    TrapCondition::Signal(crate::sys::constants::SIGINT),
                    TrapAction::Ignore,
                );
                shell.trap_actions.insert(
                    TrapCondition::Signal(crate::sys::constants::SIGTERM),
                    TrapAction::command(b"echo trapped"),
                );
                shell
                    .trap_actions
                    .insert(TrapCondition::Exit, TrapAction::command(b"echo bye"));

                shell.reset_traps_for_subshell().expect("reset");

                assert!(matches!(
                    shell.trap_action(TrapCondition::Signal(crate::sys::constants::SIGINT)),
                    Some(TrapAction::Ignore),
                ));
                assert!(
                    shell
                        .trap_action(TrapCondition::Signal(crate::sys::constants::SIGTERM))
                        .is_none(),
                );
                assert!(shell.trap_action(TrapCondition::Exit).is_none());
            },
        );
    }

    #[test]
    fn execute_trap_action_and_run_pending_traps_work() {
        run_trace(
            trace_entries![
                signal(int(sys::constants::SIGINT as i64), _) -> 0,
                signal(int(sys::constants::SIGINT as i64), _) -> 0,
                signal(int(sys::constants::SIGTERM as i64), _) -> 0,
            ],
            || {
                let mut shell = test_shell();
                let exit9 =
                    crate::syntax::parse_with_aliases(b"exit 9", &crate::hash::ShellMap::default())
                        .unwrap();
                assert_eq!(
                    shell
                        .execute_trap_action(&exit9, 3)
                        .expect("exit trap action"),
                    9
                );
                assert!(!shell.running);
                assert_eq!(shell.last_status, 9);
                shell.running = true;

                shell
                    .set_trap(
                        TrapCondition::Signal(sys::constants::SIGINT),
                        Some(TrapAction::command(b":")),
                    )
                    .expect("trap");
                sys::test_support::with_pending_signals_for_test(&[sys::constants::SIGINT], || {
                    shell.run_pending_traps().expect("run traps");
                });
                assert_eq!(shell.last_status, 9);

                shell
                    .set_trap(
                        TrapCondition::Signal(sys::constants::SIGINT),
                        Some(TrapAction::command(b"exit 7")),
                    )
                    .expect("exit trap");
                sys::test_support::with_pending_signals_for_test(&[sys::constants::SIGINT], || {
                    shell.run_pending_traps().expect("run exit trap");
                });
                assert!(!shell.running);
                shell.running = true;

                shell
                    .set_trap(
                        TrapCondition::Signal(sys::constants::SIGTERM),
                        Some(TrapAction::Ignore),
                    )
                    .expect("ignore trap");
                sys::test_support::with_pending_signals_for_test(
                    &[sys::constants::SIGTERM],
                    || {
                        shell.run_pending_traps().expect("ignored pending");
                    },
                );
            },
        );
    }

    #[test]
    fn set_trap_noop_when_signal_ignored_on_entry() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let cond = TrapCondition::Signal(sys::constants::SIGQUIT);
            shell.ignored_on_entry.insert(cond);
            shell
                .set_trap(cond, Some(TrapAction::command(b"echo trapped")))
                .expect("set_trap");
            assert!(shell.trap_action(cond).is_none());
        });
    }

    #[test]
    fn restore_signals_for_child_resets_interactive_and_monitor_signals() {
        // No user traps, not inherited-ignored: every signal in the
        // interactive set (SIGTERM/SIGQUIT/SIGINT) and the monitor
        // set (SIGTSTP/SIGTTIN/SIGTTOU) must be reset to default.
        run_trace(
            trace_entries![
                signal(int(sys::constants::SIGTERM as i64), _) -> 0,
                signal(int(sys::constants::SIGQUIT as i64), _) -> 0,
                signal(int(sys::constants::SIGINT as i64), _) -> 0,
                signal(int(sys::constants::SIGTSTP as i64), _) -> 0,
                signal(int(sys::constants::SIGTTIN as i64), _) -> 0,
                signal(int(sys::constants::SIGTTOU as i64), _) -> 0,
            ],
            || {
                let mut shell = test_shell();
                shell.interactive = true;
                shell.options.monitor = true;
                shell.restore_signals_for_child();
            },
        );
    }

    #[test]
    fn restore_signals_for_child_skips_user_trapped_ignore_signals() {
        // `trap '' SIGTERM` ran *after* startup: SIGTERM is in
        // `trap_actions` as `Ignore` but not in `ignored_on_entry`.
        // POSIX § 2.14 keeps that signal ignored across `fork+exec`,
        // so the trace contains only SIGQUIT, SIGINT, and the
        // monitor-mode trio — SIGTERM is absent.
        run_trace(
            trace_entries![
                signal(int(sys::constants::SIGQUIT as i64), _) -> 0,
                signal(int(sys::constants::SIGINT as i64), _) -> 0,
                signal(int(sys::constants::SIGTSTP as i64), _) -> 0,
                signal(int(sys::constants::SIGTTIN as i64), _) -> 0,
                signal(int(sys::constants::SIGTTOU as i64), _) -> 0,
            ],
            || {
                let mut shell = test_shell();
                shell.interactive = true;
                shell.options.monitor = true;
                shell.trap_actions.insert(
                    TrapCondition::Signal(sys::constants::SIGTERM),
                    TrapAction::Ignore,
                );
                shell.restore_signals_for_child();
            },
        );
    }

    #[test]
    fn restore_signals_for_child_resets_inherited_ignored_signals() {
        // Inherited-ignored case: `ignored_on_entry` contains SIGTSTP
        // (e.g. the parent ksh on OpenBSD propagates `SIG_IGN` for
        // job-control signals), and `Shell::probe_ignored_signals`
        // seeded `trap_actions[SIGTSTP] = Ignore` accordingly. The
        // closure must distinguish that from a user-set ignore and
        // reset SIGTSTP to default for monitor-mode children — that's
        // what makes Ctrl-Z stop the foreground job.
        run_trace(
            trace_entries![
                signal(int(sys::constants::SIGTSTP as i64), _) -> 0,
                signal(int(sys::constants::SIGTTIN as i64), _) -> 0,
                signal(int(sys::constants::SIGTTOU as i64), _) -> 0,
            ],
            || {
                let mut shell = test_shell();
                shell.options.monitor = true;
                let cond = TrapCondition::Signal(sys::constants::SIGTSTP);
                shell.ignored_on_entry.insert(cond);
                shell.trap_actions.insert(cond, TrapAction::Ignore);
                shell.restore_signals_for_child();
            },
        );
    }

    #[test]
    fn restore_signals_for_child_noninteractive_nonmonitor_is_noop() {
        // Default subshell: neither interactive nor monitor mode
        // active, so `restore_signals_for_child` issues no syscalls.
        // This covers the early-exit fall-through where neither
        // branch's signal-set is iterated.
        assert_no_syscalls(|| {
            let shell = test_shell();
            shell.restore_signals_for_child();
        });
    }
}
