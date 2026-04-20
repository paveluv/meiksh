//! POSIX-ish shell runtime: options, environment, jobs, traps, and execution loop.

mod env;
pub(crate) mod error;
mod expand_context;
pub(crate) mod jobs;
pub(crate) mod options;
pub mod run;
pub(crate) mod state;
pub(crate) mod traps;
pub(crate) mod vars;

#[cfg(test)]
pub(crate) mod test_support;
