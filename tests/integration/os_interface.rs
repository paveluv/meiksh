use super::common::*;
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
    assert!(stdout.contains("m"), "times output should contain minutes: {stdout}");
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
        .args([
            "-c",
            "trap 'echo caught' USR1; kill -USR1 $$; wait",
        ])
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
