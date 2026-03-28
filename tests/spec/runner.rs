use std::fs;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

const TEST_TIMEOUT: Duration = Duration::from_secs(5);

fn meiksh() -> &'static str {
    env!("CARGO_BIN_EXE_meiksh")
}

fn spec_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/spec")
}

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(test_name: &str) -> Self {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("meiksh-spec-{test_name}-{ts}-{seq}"));
        fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

enum TestResult {
    Pass,
    Fail(String),
    Timeout,
}

fn run_spec_test(script_path: &Path) -> TestResult {
    let test_name = script_path
        .file_stem()
        .unwrap()
        .to_string_lossy()
        .to_string();
    let tmp = TempDir::new(&test_name);
    let meiksh_path = meiksh();
    let meiksh_dir = Path::new(meiksh_path).parent().unwrap();
    let path_env = match std::env::var("PATH") {
        Ok(p) => format!("{}:{p}", meiksh_dir.display()),
        Err(_) => meiksh_dir.display().to_string(),
    };

    let mut cmd = Command::new(meiksh_path);
    cmd.arg(script_path)
        .env("TMPDIR", &tmp.path)
        .env("SHELL", meiksh_path)
        .env("PATH", &path_env)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    unsafe {
        cmd.pre_exec(|| {
            libc::setpgid(0, 0);
            Ok(())
        });
    }

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return TestResult::Fail(format!("failed to spawn meiksh: {e}")),
    };

    let pid = child.id() as libc::pid_t;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if start.elapsed() > TEST_TIMEOUT {
                    unsafe { libc::kill(-pid, libc::SIGKILL); }
                    let _ = child.wait();
                    return TestResult::Timeout;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return TestResult::Fail(format!("wait error: {e}")),
        }
    }

    let output = match child.wait_with_output() {
        Ok(o) => o,
        Err(e) => return TestResult::Fail(format!("output error: {e}")),
    };

    if output.status.success() {
        TestResult::Pass
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let code = output.status.code().unwrap_or(-1);
        TestResult::Fail(format!("exit code {code}: {stderr}"))
    }
}

fn discover_spec_tests() -> Vec<PathBuf> {
    let dir = spec_dir();
    let mut tests: Vec<PathBuf> = fs::read_dir(&dir)
        .expect("read tests/spec/")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension().is_some_and(|ext| ext == "sh")
                && p.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("SHALL-")
        })
        .collect();
    tests.sort();
    tests
}

#[test]
fn spec_compliance() {
    let filter = std::env::var("SPEC_TEST").ok();
    let tests = discover_spec_tests();

    if tests.is_empty() {
        panic!("no spec test scripts found in tests/spec/");
    }

    let mut passed = 0usize;
    let mut failed = Vec::new();
    let mut skipped = 0usize;

    for test_path in &tests {
        let name = test_path.file_stem().unwrap().to_string_lossy().to_string();

        if let Some(ref f) = filter {
            if !name.contains(f.as_str()) {
                skipped += 1;
                continue;
            }
        }

        eprint!("spec {name} ... ");

        match run_spec_test(test_path) {
            TestResult::Pass => {
                eprintln!("ok");
                passed += 1;
            }
            TestResult::Fail(msg) => {
                eprintln!("FAIL");
                eprintln!("  {msg}");
                failed.push(name);
            }
            TestResult::Timeout => {
                eprintln!("TIMEOUT ({}s)", TEST_TIMEOUT.as_secs());
                failed.push(name);
            }
        }
    }

    let total = passed + failed.len() + skipped;
    eprintln!(
        "\n--- spec tests: {passed} passed, {} failed, {skipped} skipped, {total} total ---",
        failed.len()
    );

    if !failed.is_empty() {
        eprintln!("\nFailed tests:");
        for name in &failed {
            eprintln!("  {name}");
        }
        panic!(
            "{} spec test(s) failed out of {} run",
            failed.len(),
            passed + failed.len()
        );
    }
}
