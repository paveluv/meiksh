//! POSIX-ish shell runtime: options, environment, jobs, traps, and execution loop.

mod env;
mod error;
mod expand_context;
mod jobs;
mod options;
mod run;
mod state;
mod traps;

pub use error::{ShellError, VarError};
pub use jobs::{BlockingWaitOutcome, ChildWaitResult, Job, JobState, ReapedJobState, WaitOutcome};
pub use options::{OptionError, ShellOptions};
pub use run::run_from_env;
pub use state::{FlowSignal, PendingControl, Shell};
pub use traps::{TrapAction, TrapCondition};

#[cfg(test)]
mod test_support;
