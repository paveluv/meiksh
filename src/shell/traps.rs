use crate::sys;

use super::error::ShellError;
use super::state::Shell;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TrapCondition {
    Exit,
    Signal(sys::Pid),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrapAction {
    Ignore,
    Command(Box<[u8]>),
}

impl Shell {
    pub fn trap_action(&self, condition: TrapCondition) -> Option<&TrapAction> {
        self.trap_actions.get(&condition)
    }

    pub fn set_trap(
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
                Some(TrapAction::Ignore) => {
                    sys::ignore_signal(signal).map_err(|e| self.diagnostic_syserr(1, &e))?
                }
                Some(TrapAction::Command(_)) => sys::install_shell_signal_handler(signal)
                    .map_err(|e| self.diagnostic_syserr(1, &e))?,
                None => {
                    sys::default_signal_action(signal).map_err(|e| self.diagnostic_syserr(1, &e))?
                }
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

    pub fn reset_traps_for_subshell(&mut self) -> Result<(), ShellError> {
        if self.subshell_saved_traps.is_none() {
            self.subshell_saved_traps = Some(self.trap_actions.clone());
        }
        let to_reset: Vec<TrapCondition> = self
            .trap_actions
            .iter()
            .filter_map(|(cond, action)| match action {
                TrapAction::Command(_) => Some(*cond),
                TrapAction::Ignore => None,
            })
            .collect();
        for cond in to_reset {
            if let TrapCondition::Signal(signal) = cond {
                sys::default_signal_action(signal).map_err(|e| self.diagnostic_syserr(1, &e))?;
            }
            self.trap_actions.remove(&cond);
        }
        Ok(())
    }

    pub fn restore_signals_for_child(&self) {
        let user_ignored = |sig: i32| -> bool {
            matches!(
                self.trap_actions.get(&TrapCondition::Signal(sig)),
                Some(TrapAction::Ignore)
            )
        };
        if self.interactive {
            for sig in [sys::SIGTERM, sys::SIGQUIT] {
                if !user_ignored(sig) {
                    let _ = sys::default_signal_action(sig);
                }
            }
            if !user_ignored(sys::SIGINT) {
                let _ = sys::default_signal_action(sys::SIGINT);
            }
        }
        if self.options.monitor {
            for sig in [sys::SIGTSTP, sys::SIGTTIN, sys::SIGTTOU] {
                if !user_ignored(sig) {
                    let _ = sys::default_signal_action(sig);
                }
            }
        }
    }

    pub fn run_pending_traps(&mut self) -> Result<(), ShellError> {
        for signal in sys::take_pending_signals() {
            let Some(TrapAction::Command(action)) = self
                .trap_actions
                .get(&TrapCondition::Signal(signal))
                .cloned()
            else {
                continue;
            };
            self.execute_trap_action(&action, self.last_status)?;
            if !self.running {
                break;
            }
        }
        Ok(())
    }

    pub(crate) fn run_exit_trap(&mut self, status: i32) -> Result<i32, ShellError> {
        let Some(TrapAction::Command(action)) =
            self.trap_actions.get(&TrapCondition::Exit).cloned()
        else {
            self.last_status = status;
            return Ok(status);
        };
        self.execute_trap_action(&action, status)
    }

    pub(super) fn execute_trap_action(
        &mut self,
        action: &[u8],
        preserved_status: i32,
    ) -> Result<i32, ShellError> {
        let saved_lineno = self.lineno;
        let was_running = self.running;
        self.running = true;
        self.last_status = preserved_status;
        let status = self.execute_string(action)?;
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
    use crate::sys;
    use crate::sys::test_support::{assert_no_syscalls, run_trace};
    use crate::trace_entries;

    use super::{TrapAction, TrapCondition};
    use crate::shell::test_support::test_shell;

    #[test]
    fn set_trap_ignore_and_default_use_signal_syscall() {
        run_trace(
            trace_entries![
                signal(int(sys::SIGTERM as i64), _) -> 0,
                signal(int(sys::SIGTERM as i64), _) -> 0,
            ],
            || {
                let mut shell = test_shell();
                shell
                    .set_trap(
                        TrapCondition::Signal(sys::SIGTERM),
                        Some(TrapAction::Ignore),
                    )
                    .expect("ignore");
                assert!(matches!(
                    shell.trap_action(TrapCondition::Signal(sys::SIGTERM)),
                    Some(TrapAction::Ignore)
                ));
                shell
                    .set_trap(TrapCondition::Signal(sys::SIGTERM), None)
                    .expect("default");
                assert!(
                    shell
                        .trap_action(TrapCondition::Signal(sys::SIGTERM))
                        .is_none()
                );
            },
        );
    }

    #[test]
    fn reset_traps_for_subshell_keeps_ignore_removes_command() {
        run_trace(
            trace_entries![
                signal(int(crate::sys::SIGTERM as i64), _) -> 0,
            ],
            || {
                let mut shell = test_shell();
                shell.trap_actions.insert(
                    TrapCondition::Signal(crate::sys::SIGINT),
                    TrapAction::Ignore,
                );
                shell.trap_actions.insert(
                    TrapCondition::Signal(crate::sys::SIGTERM),
                    TrapAction::Command(b"echo trapped"[..].into()),
                );
                shell.trap_actions.insert(
                    TrapCondition::Exit,
                    TrapAction::Command(b"echo bye"[..].into()),
                );

                shell.reset_traps_for_subshell().expect("reset");

                assert_eq!(
                    shell.trap_action(TrapCondition::Signal(crate::sys::SIGINT)),
                    Some(&TrapAction::Ignore),
                );
                assert_eq!(
                    shell.trap_action(TrapCondition::Signal(crate::sys::SIGTERM)),
                    None,
                );
                assert_eq!(shell.trap_action(TrapCondition::Exit), None);
            },
        );
    }

    #[test]
    fn execute_trap_action_and_run_pending_traps_work() {
        run_trace(
            trace_entries![
                signal(int(sys::SIGINT as i64), _) -> 0,
                signal(int(sys::SIGINT as i64), _) -> 0,
                signal(int(sys::SIGTERM as i64), _) -> 0,
            ],
            || {
                let mut shell = test_shell();
                assert_eq!(
                    shell
                        .execute_trap_action(b"exit 9", 3)
                        .expect("exit trap action"),
                    9
                );
                assert!(!shell.running);
                assert_eq!(shell.last_status, 9);
                shell.running = true;

                shell
                    .set_trap(
                        TrapCondition::Signal(sys::SIGINT),
                        Some(TrapAction::Command(b":"[..].into())),
                    )
                    .expect("trap");
                sys::test_support::with_pending_signals_for_test(&[sys::SIGINT], || {
                    shell.run_pending_traps().expect("run traps");
                });
                assert_eq!(shell.last_status, 9);

                shell
                    .set_trap(
                        TrapCondition::Signal(sys::SIGINT),
                        Some(TrapAction::Command(b"exit 7"[..].into())),
                    )
                    .expect("exit trap");
                sys::test_support::with_pending_signals_for_test(&[sys::SIGINT], || {
                    shell.run_pending_traps().expect("run exit trap");
                });
                assert!(!shell.running);
                shell.running = true;

                shell
                    .set_trap(
                        TrapCondition::Signal(sys::SIGTERM),
                        Some(TrapAction::Ignore),
                    )
                    .expect("ignore trap");
                sys::test_support::with_pending_signals_for_test(&[sys::SIGTERM], || {
                    shell.run_pending_traps().expect("ignored pending");
                });
            },
        );
    }

    #[test]
    fn set_trap_noop_when_signal_ignored_on_entry() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let cond = TrapCondition::Signal(sys::SIGQUIT);
            shell.ignored_on_entry.insert(cond);
            shell
                .set_trap(cond, Some(TrapAction::Command(b"echo trapped"[..].into())))
                .expect("set_trap");
            assert!(shell.trap_action(cond).is_none());
        });
    }
}
