#![allow(
    clippy::disallowed_types,
    clippy::disallowed_macros,
    clippy::disallowed_methods
)]

mod sys;

mod common;
mod interactive_common;

mod bind_builtin;
mod builtins;
mod control_flow;
mod emacs_mode;
mod expansion;
mod inputrc_parser;
mod interactive;
mod os_interface;
mod parser_coverage;
mod prompt;
mod redirection;
mod shell_options;

use common::*;
use std::process::Command;

// ── Core execution ──

#[test]
fn executes_simple_command_string() {
    let output = Command::new(meiksh())
        .args(["-c", "printf hi"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "hi");
}

#[test]
fn executes_pipeline() {
    let output = Command::new(meiksh())
        .args(["-c", "printf hi | wc -c"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "2");
}

#[test]
fn handles_background_wait() {
    let output = Command::new(meiksh())
        .args(["-c", "sleep 0.1 & wait"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
}
