use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

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

fn run_spec_test(script_path: &Path) -> Result<(), String> {
    let test_name = script_path
        .file_stem()
        .unwrap()
        .to_string_lossy()
        .to_string();
    let tmp = TempDir::new(&test_name);
    let meiksh_path = meiksh();

    let output = Command::new(meiksh_path)
        .arg(script_path)
        .env("TMPDIR", &tmp.path)
        .env("SHELL", meiksh_path)
        .output()
        .map_err(|e| format!("failed to spawn meiksh: {e}"))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let code = output.status.code().unwrap_or(-1);
        Err(format!("exit code {code}: {stderr}"))
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

        match run_spec_test(test_path) {
            Ok(()) => passed += 1,
            Err(msg) => {
                eprintln!("FAIL {name}: {msg}");
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
