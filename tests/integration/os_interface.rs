use super::common::*;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

#[test]
fn env_roundtrip_export_echo_unset() {
    let out = Command::new(meiksh())
        .args([
            "-c",
            "export MEIKSH_IT_97a3=hello; printf '%s' \"$MEIKSH_IT_97a3\"; unset MEIKSH_IT_97a3; printf '|%s|' \"$MEIKSH_IT_97a3\"",
        ])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), "hello||");
}

#[test]
fn classify_byte_bracket_expressions() {
    let out = Command::new(meiksh())
        .args([
            "-c",
            concat!(
                "case a in [[:alpha:]]) printf A;; esac; ",
                "case 5 in [[:digit:]]) printf D;; esac; ",
                "case ' ' in [[:blank:]]) printf B;; esac; ",
                "case . in [[:punct:]]) printf P;; esac; ",
                "case Z in [[:upper:]]) printf U;; esac; ",
                "case z in [[:lower:]]) printf L;; esac; ",
                "case f in [[:xdigit:]]) printf X;; esac; ",
            ),
        ])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), "ADBPULX");
}

#[test]
fn lstat_and_unlink_via_file_test_and_rm() {
    let tmp = TempDir::new("os-iface-lstat");
    let f = tmp.join("probe");
    std::fs::write(&f, "x").expect("create");

    let script = format!(
        "[ -f '{}' ] && rm '{}' && [ ! -f '{}' ] && echo ok",
        f.display(),
        f.display(),
        f.display()
    );
    let out = Command::new(meiksh())
        .args(["-c", &script])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "ok");
}

#[test]
fn strcoll_string_comparison() {
    let out = Command::new(meiksh())
        .args([
            "-c",
            "[ abc \\< abd ] && printf L; [ abd \\> abc ] && printf G; [ abc = abc ] && printf E",
        ])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), "LGE");
}

#[test]
fn decode_and_encode_multibyte_char() {
    let out = Command::new(meiksh())
        .args(["-c", "printf '%s' é | wc -m"])
        .env("LC_ALL", "C.UTF-8")
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "1");
}

#[test]
fn times_builtin_exercises_times_and_sysconf() {
    let out = Command::new(meiksh())
        .args(["-c", "times"])
        .output()
        .expect("run");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("m"),
        "times output should contain minutes: {stdout}"
    );
}

#[test]
fn decimal_point_in_arithmetic() {
    let out = Command::new(meiksh())
        .args(["-c", "echo $((1 + 2))"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "3");
}

#[test]
fn mb_cur_max_allows_multibyte_in_utf8() {
    let out = Command::new(meiksh())
        .args(["-c", "printf '%s' 日本語"])
        .env("LC_ALL", "C.UTF-8")
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), "日本語");
}

#[test]
fn signal_trap_and_pending_bits() {
    let out = Command::new(meiksh())
        .args(["-c", "trap 'echo caught' USR1; kill -USR1 $$; wait"])
        .output()
        .expect("run");
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "caught");
}

#[test]
fn execve_error_for_nonexistent_command() {
    let out = Command::new(meiksh())
        .args(["-c", "/nonexistent_meiksh_cmd_xyz 2>/dev/null; echo $?"])
        .output()
        .expect("run");
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "127");
}

// Run `exec FILE` on a file that libc can open but refuses to run
// (ENOEXEC — empty file with +x bit but no shebang, no ELF magic).
// This exercises `sys::interface::execve` in the production binary:
// the exec builtin calls `exec_replace_with_env`, which flushes coverage
// and then calls `libc::execve`; execve returns -1 with errno=ENOEXEC,
// so the counter for the function entry survives and is written out.
// A successful `exec /bin/echo hi` would replace the process image
// before the execve entry counter could be persisted.
#[test]
fn exec_builtin_reports_enoexec_for_empty_executable() {
    let tmp = TempDir::new("meiksh-execve-enoexec");
    let bogus = tmp.join("bogus");
    std::fs::write(&bogus, "").expect("write bogus");
    // Make it executable so which_in_path doesn't reject it and we
    // actually reach libc::execve in the exec builtin path.
    let mut perms = std::fs::metadata(&bogus).expect("meta").permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&bogus, perms).expect("chmod");

    // POSIX requires a non-interactive shell to exit when `exec`'s
    // replacement call fails, so we cannot observe `$?` after exec.
    // We only assert on the diagnostic and the nonzero status.
    let script = format!("exec {}", bogus.display());
    let out = Command::new(meiksh())
        .args(["-c", &script])
        .output()
        .expect("run meiksh");
    assert!(
        !out.status.success(),
        "shell should exit nonzero after failed exec",
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("Exec format error"),
        "expected ENOEXEC diagnostic on stderr, got {stderr:?}",
    );
}

// `time cmd` runs the pipeline-timed path that reads
// `sys::time::monotonic_clock_ns()` before and after the command;
// that delegates to `sys::interface::monotonic_clock_ns`, the
// CLOCK_MONOTONIC libc wrapper that is otherwise unreachable from
// non-interactive integration tests.
#[test]
fn time_builtin_reports_real_and_user_sys_lines() {
    let out = Command::new(meiksh())
        .args(["-c", "time true"])
        .output()
        .expect("run meiksh");
    assert!(out.status.success(), "time true should succeed");
    let stderr = String::from_utf8_lossy(&out.stderr);
    // POSIX `time` writes three lines (real/user/sys). We don't check
    // the numeric values (kernel-timing-dependent), just the structure
    // so the assertion points at the monotonic_clock_ns wrapper and
    // not at some other format regression.
    assert!(
        stderr.contains("real") && stderr.contains("user") && stderr.contains("sys"),
        "time output should contain real/user/sys lines, got {stderr:?}"
    );
}

// `test -L path` (and its alias `-h`) is the only code path that drives
// `sys::fs::lstat_path`, which wraps `sys::interface::lstat`. Create
// a real symlink so lstat reports S_IFLNK and the builtin returns
// true; `test -L` on a plain file or missing path must be false.
#[test]
fn test_builtin_dash_l_detects_symlink_via_lstat() {
    let tmp = TempDir::new("meiksh-lstat");
    let target = tmp.join("target.txt");
    let link = tmp.join("link");
    let plain = tmp.join("plain");
    std::fs::write(&target, "x").expect("target");
    std::fs::write(&plain, "y").expect("plain");
    std::os::unix::fs::symlink(&target, &link).expect("symlink");

    let script = format!(
        "[ -L '{link}' ] && printf Y || printf N; \
         [ -L '{plain}' ] && printf Y || printf N; \
         [ -L '{missing}' ] && printf Y || printf N",
        link = link.display(),
        plain = plain.display(),
        missing = tmp.join("missing").display(),
    );
    let out = Command::new(meiksh())
        .args(["-c", &script])
        .output()
        .expect("run meiksh");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), "YNN");
}

// POSIX `[[:cntrl:]]`, `[[:graph:]]`, `[[:print:]]`, `[[:space:]]`
// routes through `sys::locale::classify_wchar` for each class; under
// LC_ALL=C.UTF-8 the shell uses the libc `iswcntrl`/`iswgraph`/
// `iswprint`/`iswspace` helpers rather than the ASCII fast path.
// This test asserts both positive (match) and negative (no match)
// outcomes for each class so a regression that short-circuited any
// single branch to `true` or `false` would flip the assertion.
#[test]
fn case_pattern_covers_cntrl_graph_print_space_classes() {
    // Use $'…' style escapes via printf so the script stays pure ASCII
    // here but the classified bytes include control / whitespace.
    let script = r#"
        LC_ALL=C.UTF-8
        tab=$(printf '\t')
        bell=$(printf '\007')
        # cntrl: BEL is control, 'a' is not
        case "$bell" in [[:cntrl:]]) printf c1;; *) printf c0;; esac
        case a      in [[:cntrl:]]) printf c1;; *) printf c0;; esac
        # graph: 'a' is graphic, space is not
        case a      in [[:graph:]]) printf g1;; *) printf g0;; esac
        case ' '    in [[:graph:]]) printf g1;; *) printf g0;; esac
        # print: ' ' is printable, BEL is not
        case ' '    in [[:print:]]) printf p1;; *) printf p0;; esac
        case "$bell" in [[:print:]]) printf p1;; *) printf p0;; esac
        # space: tab is space, 'a' is not
        case "$tab" in [[:space:]]) printf s1;; *) printf s0;; esac
        case a      in [[:space:]]) printf s1;; *) printf s0;; esac
    "#;
    let out = Command::new(meiksh())
        .args(["-c", script])
        .env("LC_ALL", "C.UTF-8")
        .output()
        .expect("run meiksh");
    assert!(
        out.status.success(),
        "case script should succeed; stderr={:?}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "c1c0g1g0p1p0s1s0",
        "each class should match only the expected category",
    );
}

// An isolated UTF-8 lead byte followed by ASCII is an invalid
// multibyte sequence: `mbrtowc(3)` returns `(size_t)-1` with EILSEQ.
// `sys::locale::decode_char_impl` treats that as "one raw byte" so
// the shell can still make progress. We observe the fallback through
// `${#x}`: the full byte string has length `1 + 4 = 5` characters
// (one invalid byte + "abcd"). If the fallback ever regressed to
// "skip the byte" (returning `(_, 0)`) the length would drop by one.
#[test]
fn decode_char_falls_back_to_raw_byte_on_invalid_utf8() {
    let out = Command::new(meiksh())
        .env("LC_ALL", "C.UTF-8")
        .args(["-c", "x=$(printf '\\303abcd'); printf '%s' \"${#x}\""])
        .output()
        .expect("run meiksh");
    assert!(
        out.status.success(),
        "stderr={:?}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "5",
        "invalid UTF-8 lead byte should count as one character",
    );
}

// With `set -m` (monitor mode) enabled even in non-interactive
// scripts, `jobs` walks the reaper which calls `try_wait_child`.
// That exercises `sys::process::wifcontinued` (for a child we have
// just `kill -CONT`ed) and `sys::process::wstopsig` (for a child we
// `kill -STOP`ed). This test sequences a SIGSTOP / SIGCONT / SIGTERM
// on a backgrounded sleep and asserts that `jobs` reports the
// Stopped-then-Running transition as meiksh sees it, which is the
// only observable side effect that uniquely exercises both wrappers.
#[test]
fn job_control_stop_then_continue_drives_wifcontinued_and_wstopsig() {
    let out = Command::new(meiksh())
        .args([
            "-m",
            "-c",
            "sleep 10 &\n\
             pid=$!\n\
             sleep 0.2\n\
             kill -STOP \"$pid\"\n\
             sleep 0.5\n\
             jobs\n\
             kill -CONT \"$pid\"\n\
             sleep 0.3\n\
             jobs\n\
             kill \"$pid\"\n\
             wait 2>/dev/null\n\
             printf done",
        ])
        .output()
        .expect("run meiksh");
    assert!(
        out.status.success(),
        "script must succeed; stderr={:?}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Two `jobs` reports: one while Stopped (with the SIGSTOP name
    // reported by `wstopsig`), one while Running again (reached via
    // `wifcontinued`). The final `printf done` separates the jobs
    // output from any trailing noise.
    assert!(
        stdout.contains("Stopped") && stdout.contains("SIGSTOP"),
        "expected `jobs` to show Stopped (SIGSTOP) state; got {stdout:?}"
    );
    assert!(
        stdout.contains("Running"),
        "expected `jobs` to show Running after SIGCONT; got {stdout:?}"
    );
    assert!(
        stdout.ends_with("done"),
        "script must reach `done` sentinel"
    );
}

// When the shell has no `PATH` in its internal variable store **and**
// the invoked program name is a bare identifier (no `/`), PATH lookup
// in `exec::process::resolve_command_path` falls through to
// `sys::env::env_var(b"PATH")`, which bottoms out in the production
// `sys::interface::getenv` wrapper. We drive the `ptr.is_null()`
// branch by removing PATH from the parent environment and invoking a
// non-existent program by bare name: the shell tries shell-var PATH
// (None), then env PATH (None), ends up with an empty search path,
// prints a `not found` diagnostic, and still runs the next command.
//
// Absolute paths like `/bin/echo` must NOT be used here — the
// resolver short-circuits on any program name containing `/` (see
// `resolve_command_path` in `exec/process.rs`), bypassing the getenv
// fallback we want to cover.
#[test]
fn getenv_null_branch_fires_when_path_is_fully_unset() {
    let out = Command::new(meiksh())
        .env_remove("PATH")
        .args(["-c", "unset PATH; no_such_cmd_xyz 2>/dev/null; printf ok"])
        .output()
        .expect("run meiksh");
    assert!(
        out.status.success(),
        "shell should keep running after the failed lookup; stderr={:?}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "ok",
        "the sentinel `printf ok` must fire after the missing command"
    );
}

// A POSIX bracket expression may name any class (`[[:foo:]]`) — only
// twelve are enumerated inline in `sys::locale::classify_wchar`; any
// other name must fall through to `classify_wchar_wctype`, which asks
// libc's `wctype(3)` to look the class up at runtime. `wctype` returns
// 0 for an unrecognized name (POSIX.1-2017, `wctype(3)`), and our
// wrapper then returns `false`. The assertion below pins down that
// contract end-to-end: if the fallback ever regressed to `true`, the
// "ok" branch would flip, and if we ever added `weird` as a real class
// it would need a deliberate update.
#[test]
fn classify_wchar_rejects_unknown_bracket_class() {
    let out = Command::new(meiksh())
        .env("LC_ALL", "C.UTF-8")
        .args([
            "-c",
            "case a in [[:weird:]]) printf match;; *) printf nomatch;; esac",
        ])
        .output()
        .expect("run meiksh");
    assert!(
        out.status.success(),
        "case script must succeed even with unknown class; stderr={:?}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "nomatch",
        "unknown bracket class must never match any character",
    );
}

// Multi-byte input in a glob pattern forces `decode_char_impl` in
// production: the pattern matcher calls `sys::locale::decode_char`
// on non-ASCII bytes, which skips the ASCII fast path and the
// `mb_cur_max == 1` fast path when LC_ALL=C.UTF-8 is active, ending
// up in `mbrtowc`. The test matches a multi-byte character against
// both a literal and a character-class pattern to assert that the
// libc decode returns the full code point (not byte-by-byte).
#[test]
fn utf8_multibyte_matches_in_case_patterns() {
    let script = r#"
        LC_ALL=C.UTF-8
        # U+00E9 'é' is 0xC3 0xA9. Literal match.
        case é in é) printf L;; *) printf l;; esac
        # Alpha class must also accept é under UTF-8 locale (iswalpha).
        case é in [[:alpha:]]) printf A;; *) printf a;; esac
        # Non-letter U+00A3 '£' must NOT match [[:alpha:]].
        case £ in [[:alpha:]]) printf A;; *) printf a;; esac
    "#;
    let out = Command::new(meiksh())
        .args(["-c", script])
        .env("LC_ALL", "C.UTF-8")
        .output()
        .expect("run meiksh");
    assert!(
        out.status.success(),
        "multibyte case script should succeed; stderr={:?}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), "LAa");
}
