use super::sys;
use std::io::Write;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

pub fn meiksh() -> &'static str {
    env!("CARGO_BIN_EXE_meiksh")
}

pub struct TempDir {
    pub path: PathBuf,
}

impl TempDir {
    pub fn new(prefix: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("{prefix}-{unique}"));
        fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    pub fn join(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

pub fn run_meiksh_with_stdin(script: &str, stdin: &[u8]) -> std::process::Output {
    let mut child = Command::new(meiksh())
        .args(["-c", script])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn meiksh");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(stdin)
        .expect("write stdin");
    child.wait_with_output().expect("wait for meiksh")
}

pub fn run_meiksh_with_nonblocking_stdin(stdin: &[u8]) -> std::process::Output {
    let mut command = Command::new(meiksh());
    unsafe {
        command.pre_exec(|| sys::set_nonblocking(0, true));
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn meiksh with nonblocking stdin");
    let mut child_stdin = child.stdin.take().expect("stdin");
    std::thread::sleep(std::time::Duration::from_millis(100));
    child_stdin.write_all(stdin).expect("write delayed stdin");
    drop(child_stdin);
    child.wait_with_output().expect("wait for meiksh")
}

pub fn run_interactive(input: &[u8]) -> std::process::Output {
    let mut child = Command::new(meiksh())
        .arg("-i")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    child.stdin.as_mut().unwrap().write_all(input).unwrap();
    child.wait_with_output().expect("wait")
}
