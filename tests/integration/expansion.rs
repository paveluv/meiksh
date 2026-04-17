use super::common::*;
use std::fs;
use std::process::Command;

// ── Parameter expansion ──

#[test]
fn expands_parameters_and_pathnames_more_like_posix() {
    let positional = Command::new(meiksh())
        .args([
            "-c",
            "set -- a b c d e f g h i j; printf '%s|%s|%s' \"$10\" \"${10}\" \"${#10}\"",
        ])
        .output()
        .expect("run meiksh");
    assert!(positional.status.success());
    assert_eq!(String::from_utf8_lossy(&positional.stdout), "a0|j|1");

    let operators = Command::new(meiksh())
        .args([
            "-c",
            "unset X; printf '<%s><%s><%s><%s>' \"${X-word}\" \"${X:-word}\" \"${X+alt}\" \"${X:+alt}\"; X=''; printf '|<%s><%s><%s><%s>' \"${X-word}\" \"${X:-word}\" \"${X+alt}\" \"${X:+alt}\"",
        ])
        .output()
        .expect("run meiksh");
    assert!(operators.status.success());
    assert_eq!(
        String::from_utf8_lossy(&operators.stdout),
        "<word><word><><>|<><word><alt><>"
    );

    let trimming = Command::new(meiksh())
        .args([
            "-c",
            "PATHNAME='src/bin/main.rs'; DOTTED='alpha.beta.gamma'; printf '%s|%s|%s|%s|%s|%s|%s|%s|%s|%s' \"${PATHNAME#*/}\" \"${PATHNAME##*/}\" \"${PATHNAME%/*}\" \"${PATHNAME%%/*}\" \"${DOTTED#*.}\" \"${DOTTED##*.}\" \"${DOTTED%.*}\" \"${DOTTED%%.*}\" \"${DOTTED#\"*.\"}\" \"${PATHNAME#\"src/\"}\"",
        ])
        .output()
        .expect("run meiksh");
    assert!(trimming.status.success());
    assert_eq!(
        String::from_utf8_lossy(&trimming.stdout),
        "bin/main.rs|main.rs|src/bin|src|beta.gamma|gamma|alpha.beta|alpha|alpha.beta.gamma|bin/main.rs"
    );

    let dir = TempDir::new("meiksh-expand-spec");
    fs::write(dir.join("a.txt"), "").expect("write a");
    fs::write(dir.join("b.txt"), "").expect("write b");
    fs::write(dir.join(".hidden.txt"), "").expect("write hidden");

    let glob = Command::new(meiksh())
        .current_dir(dir.path())
        .args(["-c", "printf '%s|' *.txt \\*.txt .*\\.txt"])
        .output()
        .expect("run meiksh");
    assert!(glob.status.success());
    assert_eq!(
        String::from_utf8_lossy(&glob.stdout),
        "a.txt|b.txt|*.txt|.hidden.txt|"
    );

    let noglob = Command::new(meiksh())
        .current_dir(dir.path())
        .args([
            "-c",
            "set -f; printf '%s|' *.txt; set +f; printf '%s|' *.txt",
        ])
        .output()
        .expect("run meiksh");
    assert!(noglob.status.success());
    assert_eq!(
        String::from_utf8_lossy(&noglob.stdout),
        "*.txt|a.txt|b.txt|"
    );

    let shell_option = Command::new(meiksh())
        .current_dir(dir.path())
        .args(["-f", "-c", "printf '%s|' *.txt"])
        .output()
        .expect("run meiksh");
    assert!(shell_option.status.success());
    assert_eq!(String::from_utf8_lossy(&shell_option.stdout), "*.txt|");
}

#[test]
fn dollar_single_quotes_follow_issue_eight_rules() {
    let output = Command::new(meiksh())
        .args([
            "-c",
            "printf '%s|%s|%s' $'a b' $'line\\nnext' \"$'literal'\"",
        ])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "a b|line\nnext|$'literal'"
    );
}

#[test]
fn field_splitting_respects_ifs_defaults_and_star_joining() {
    let output = Command::new(meiksh())
        .args([
            "-c",
            "VALUE='a b'; unset IFS; printf '<%s>' $VALUE; IFS=; printf '|<%s>' $VALUE; set -- a b c; IFS=:; printf '|<%s><%s>' $* \"$*\"",
        ])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "<a><b>|<a b>|<a><b>|<c><a:b:c>"
    );
}

#[test]
fn parameter_default_with_at_fields() {
    let output = Command::new(meiksh())
        .args([
            "-c",
            r#"set a b c; for x in ${unset:-"$@"}; do echo "($x)"; done"#,
        ])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "(a)\n(b)\n(c)"
    );

    let output2 = Command::new(meiksh())
        .args(["-c", r#"set --; echo "${unset:-"$@"}""#])
        .output()
        .expect("run meiksh");
    assert!(output2.status.success());
    assert_eq!(String::from_utf8_lossy(&output2.stdout).trim(), "");
}

#[test]
fn quoted_null_adjacent_to_empty_at_produces_one_field() {
    let output = Command::new(meiksh())
        .args(["-c", r#"set --; for x in ''"$@"; do echo "[$x]"; done"#])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "[]");

    let output2 = Command::new(meiksh())
        .args(["-c", r#"set --; for x in 'pfx'"$@"; do echo "[$x]"; done"#])
        .output()
        .expect("run meiksh");
    assert!(output2.status.success());
    assert_eq!(String::from_utf8_lossy(&output2.stdout).trim(), "[pfx]");
}

#[test]
fn invalid_parameter_expansion_reports_error() {
    let output = Command::new(meiksh())
        .args(["-c", "echo ${%bad}"])
        .output()
        .expect("run meiksh");
    assert!(!output.status.success());
}

#[test]
fn parameter_pattern_removal_operators() {
    let output = Command::new(meiksh())
        .args([
            "-c",
            r#"f=archive.tar.gz; echo "${f%.*}" "${f%%.*}" "${f#*.}" "${f##*.}""#,
        ])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "archive.tar archive tar.gz gz"
    );
}

#[test]
fn character_class_pattern_matching_uses_locale() {
    let output = Command::new(meiksh())
        .args([
            "-c",
            "case a in ([[:alpha:]]) echo yes;; (*) echo no;; esac",
        ])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "yes");
}

#[test]
fn string_to_bytes_round_trips_non_ascii() {
    let output = Command::new(meiksh())
        .args([
            "-c",
            r#"v=$(printf '\351'); printf '%s' "$v" | od -An -t x1 | tr -d ' \n'"#,
        ])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "e9");
}

#[test]
fn capture_output_preserves_non_utf8_bytes() {
    let output = Command::new(meiksh())
        .args([
            "-c",
            r#"v=$(printf 'A\377B\200C'); printf '%s' "$v" | od -An -t x1 | tr -d ' \n'"#,
        ])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "41ff428043");
}

// ── Double-quote backslash ──

#[test]
fn dquote_backslash_preserves_non_special() {
    let output = Command::new(meiksh())
        .args(["-c", r#"printf '%s\n' "\a\z""#])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), r"\a\z");
}

#[test]
fn dquote_backslash_escapes_dollar_and_backslash() {
    let output = Command::new(meiksh())
        .args(["-c", r#"echo "\$HOME""#])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "$HOME");

    let output = Command::new(meiksh())
        .args(["-c", r#"echo "\\""#])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "\\");
}

// ── Arithmetic expansion ──

#[test]
fn arithmetic_expansion_full_operators() {
    let cases: &[(&str, &str)] = &[
        ("echo $((3 + 4))", "7"),
        ("echo $((10 - 3))", "7"),
        ("echo $((3 * 4))", "12"),
        ("echo $((15 / 3))", "5"),
        ("echo $((17 % 5))", "2"),
        ("echo $((3 < 5))", "1"),
        ("echo $((5 < 3))", "0"),
        ("echo $((3 == 3))", "1"),
        ("echo $((3 != 5))", "1"),
        ("echo $((6 & 3))", "2"),
        ("echo $((6 | 3))", "7"),
        ("echo $((6 ^ 3))", "5"),
        ("echo $((~0))", "-1"),
        ("echo $((1 << 4))", "16"),
        ("echo $((16 >> 2))", "4"),
        ("echo $((1 && 1))", "1"),
        ("echo $((0 || 1))", "1"),
        ("echo $((!0))", "1"),
        ("echo $((1 ? 10 : 20))", "10"),
        ("echo $((0 ? 10 : 20))", "20"),
    ];
    for (cmd, expected) in cases {
        let output = Command::new(meiksh())
            .args(["-c", cmd])
            .output()
            .expect("run meiksh");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout.trim(), *expected, "failed for: {cmd}");
    }
}

#[test]
fn arithmetic_variable_references() {
    let output = Command::new(meiksh())
        .args(["-c", "x=7; echo $((x + 3))"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "10");

    let output = Command::new(meiksh())
        .args(["-c", "x=5; echo $(($x * 2))"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "10");
}

#[test]
fn arithmetic_hex_and_octal() {
    let output = Command::new(meiksh())
        .args(["-c", "echo $((0xff))"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "255");

    let output = Command::new(meiksh())
        .args(["-c", "echo $((010))"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "8");
}

#[test]
fn arithmetic_assignment_persists() {
    let output = Command::new(meiksh())
        .args(["-c", "x=1; y=$((x += 5)); echo $x $y"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "6 6");
}

// ── Tilde expansion ──

#[test]
fn tilde_home_expansion() {
    let output = Command::new(meiksh())
        .args(["-c", "echo ~/test"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.trim().starts_with('~'),
        "tilde should have been expanded, got: {stdout}"
    );
    assert!(
        stdout.trim().ends_with("/test"),
        "should end with /test, got: {stdout}"
    );
}

#[test]
fn tilde_user_expansion_via_getpwnam() {
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap();
    let output = Command::new(meiksh())
        .args(["-c", &format!("echo ~{user}")])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();
    assert!(
        !trimmed.starts_with('~'),
        "~{user} should have been expanded, got: {trimmed}"
    );
    assert!(
        trimmed.starts_with('/'),
        "should be an absolute path, got: {trimmed}"
    );
}

#[test]
fn tilde_unknown_user_preserved() {
    let output = Command::new(meiksh())
        .args(["-c", "echo ~no_such_user_xyzzy_12345"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "~no_such_user_xyzzy_12345");
}

#[test]
fn tilde_in_assignment_after_colon() {
    let output = Command::new(meiksh())
        .args(["-c", "MYPATH=~/bin:~/lib; echo $MYPATH"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();
    assert!(
        !trimmed.contains('~'),
        "tildes should have been expanded, got: {trimmed}"
    );
    assert!(
        trimmed.contains(':'),
        "should have colon separator, got: {trimmed}"
    );
}

#[test]
fn tilde_null_home_produces_empty_field() {
    let output = Command::new(meiksh())
        .args(["-c", "HOME=''; set -- ~; echo $#; printf '<%s>\\n' \"$1\""])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "1\n<>");
}

#[test]
fn tilde_unset_home_stays_literal() {
    let output = Command::new(meiksh())
        .args(["-c", "unset HOME; echo ~"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "~");
}

#[test]
fn tilde_not_expanded_with_non_login_chars() {
    let output = Command::new(meiksh())
        .args(["-c", "HOME=/my/home; echo ~$(echo /foo)"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "~/foo");
}

#[test]
fn export_with_tilde_prefix_expansion() {
    let output = Command::new(meiksh())
        .args(["-c", "HOME=/fakehome; export V=~/bin; echo $V"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "/fakehome/bin"
    );
}

#[test]
fn export_with_known_tilde_user_expansion() {
    let output = Command::new(meiksh())
        .args(["-c", "export V=~root/bin; echo $V"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.trim().ends_with("/bin"),
        "expected ~root/bin to expand, got: {stdout}"
    );
    assert!(!stdout.contains('~'), "tilde should have been expanded");
}

#[test]
fn export_with_unknown_tilde_user_preserved() {
    let output = Command::new(meiksh())
        .args(["-c", "export V=~no_such_user_xyzzy_999/bin; echo $V"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "~no_such_user_xyzzy_999/bin"
    );
}

#[test]
fn assignment_with_nested_parameter_expansion() {
    let output = Command::new(meiksh())
        .args(["-c", "y=${x:-hello}; echo $y"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "hello");
}

#[test]
fn backslash_newline_continuation_in_tokenizer() {
    let output = Command::new(meiksh())
        .args(["-c", "echo hel\\\nlo"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "hello");
}

// ── Coverage tests for WordPart IR parser and parts-based expander ──

#[test]
fn dollar_single_quote_escapes() {
    let out = Command::new(meiksh())
        .args([
            "-c",
            r#"printf '%s\n' $'\a\b\e\f\n\r\t\v\"\'\\\x41\077\cA'"#,
        ])
        .output()
        .expect("run");
    assert!(out.status.success());
    let bytes = &out.stdout;
    assert_eq!(bytes[0], 0x07); // \a
    assert_eq!(bytes[1], 0x08); // \b
    assert_eq!(bytes[2], 0x1b); // \e
    assert_eq!(bytes[3], 0x0c); // \f
    assert_eq!(bytes[4], b'\n'); // \n
    assert_eq!(bytes[5], b'\r'); // \r
    assert_eq!(bytes[6], b'\t'); // \t
    assert_eq!(bytes[7], 0x0b); // \v
    assert_eq!(bytes[8], b'"'); // \"
    assert_eq!(bytes[9], b'\''); // \'
    assert_eq!(bytes[10], b'\\'); // \\
    assert_eq!(bytes[11], b'A'); // \x41
    assert_eq!(bytes[12], b'?'); // \077
    assert_eq!(bytes[13], 0x01); // \cA
}

#[test]
fn special_vars_via_parts() {
    let out = Command::new(meiksh())
        .args(["-c", "set -- a b c; echo $# $? $$ $- $0"])
        .output()
        .expect("run");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parts: Vec<&str> = stdout.trim().split_whitespace().collect();
    assert_eq!(parts[0], "3"); // $#
    assert_eq!(parts[1], "0"); // $?
    assert!(!parts[2].is_empty()); // $$
    assert!(!parts[3].is_empty()); // $-
}

#[test]
fn positional_params_via_parts() {
    let out = Command::new(meiksh())
        .args(["-c", "set -- alpha beta; echo $1 $2"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "alpha beta");
}

#[test]
fn at_star_expansion_via_parts() {
    let out = Command::new(meiksh())
        .args(["-c", "set -- x y z; echo \"$@\"; echo $*"])
        .output()
        .expect("run");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines[0], "x y z");
    assert_eq!(lines[1], "x y z");
}

#[test]
fn star_with_custom_ifs() {
    let out = Command::new(meiksh())
        .args(["-c", "IFS=:; set -- a b c; echo \"$*\""])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "a:b:c");
}

#[test]
fn literal_dollar_via_parts() {
    let out = Command::new(meiksh())
        .args(["-c", "echo $ alone"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "$ alone");
}

#[test]
fn braced_default_op_via_parts() {
    let out = Command::new(meiksh())
        .args(["-c", "unset x; echo ${x:-fallback}"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "fallback");
}

#[test]
fn braced_default_colon_empty_via_parts() {
    let out = Command::new(meiksh())
        .args(["-c", "x=''; echo ${x:-notempty}"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "notempty");
}

#[test]
fn braced_assign_op_via_parts() {
    let out = Command::new(meiksh())
        .args(["-c", "unset x; echo ${x:=assigned}; echo $x"])
        .output()
        .expect("run");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines[0], "assigned");
    assert_eq!(lines[1], "assigned");
}

#[test]
fn braced_error_op_via_parts() {
    let out = Command::new(meiksh())
        .args(["-c", "unset x; echo ${x?custom error msg} 2>&1; echo done"])
        .output()
        .expect("run");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("custom error msg"),
        "expected error msg in stderr: {stderr}"
    );
}

#[test]
fn braced_error_colon_op_via_parts() {
    let out = Command::new(meiksh())
        .args(["-c", "x=''; echo ${x:?must not be empty} 2>&1; echo done"])
        .output()
        .expect("run");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("must not be empty"),
        "expected error msg in stderr: {stderr}"
    );
}

#[test]
fn braced_alt_op_via_parts() {
    let out = Command::new(meiksh())
        .args([
            "-c",
            "x=val; echo \"${x+alt}\"; unset x; echo \"${x+gone}end\"",
        ])
        .output()
        .expect("run");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines[0], "alt");
    assert_eq!(lines[1], "end");
}

#[test]
fn braced_alt_colon_op_via_parts() {
    let out = Command::new(meiksh())
        .args(["-c", "x=''; echo \"${x:+notempty}end\""])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "end");
}

#[test]
fn braced_length_op_via_parts() {
    let out = Command::new(meiksh())
        .args(["-c", "x=hello; echo ${#x}"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "5");
}

#[test]
fn braced_trim_suffix_via_parts() {
    let out = Command::new(meiksh())
        .args(["-c", "v=a/b/c.txt; echo ${v%.*}; echo ${v%%/*}"])
        .output()
        .expect("run");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines[0], "a/b/c");
    assert_eq!(lines[1], "a");
}

#[test]
fn braced_trim_prefix_via_parts() {
    let out = Command::new(meiksh())
        .args(["-c", "v=a/b/c.txt; echo ${v#*/}; echo ${v##*/}"])
        .output()
        .expect("run");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines[0], "b/c.txt");
    assert_eq!(lines[1], "c.txt");
}

#[test]
fn braced_positional_param() {
    let out = Command::new(meiksh())
        .args(["-c", "set -- one two; echo ${1} ${2}"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "one two");
}

#[test]
fn braced_special_param() {
    let out = Command::new(meiksh())
        .args(["-c", "set -- a b; echo ${#}; echo ${?}"])
        .output()
        .expect("run");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines[0], "2");
    assert_eq!(lines[1], "0");
}

#[test]
fn arithmetic_literal_via_parts() {
    let out = Command::new(meiksh())
        .args(["-c", "echo $((42))"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "42");
}

#[test]
fn arithmetic_with_var_via_parts() {
    let out = Command::new(meiksh())
        .args(["-c", "x=10; echo $((x + $x * 2))"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "30");
}

#[test]
fn command_sub_via_parts() {
    let out = Command::new(meiksh())
        .args(["-c", "echo $(echo hello)"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello");
}

#[test]
fn backtick_sub_via_parts() {
    let out = Command::new(meiksh())
        .args(["-c", "echo `echo world`"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "world");
}

#[test]
fn backtick_in_double_quotes() {
    let out = Command::new(meiksh())
        .args(["-c", "echo \"`echo inner`\""])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "inner");
}

#[test]
fn backtick_with_escape_in_dquotes() {
    let out = Command::new(meiksh())
        .args(["-c", r#"echo "`echo \"hi\"`""#])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hi");
}

#[test]
fn backtick_with_dollar_escape() {
    let out = Command::new(meiksh())
        .args(["-c", "echo `echo \\\\\\$HOME`"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "$HOME");
}

#[test]
fn double_quote_parts_with_backslash() {
    let out = Command::new(meiksh())
        .args(["-c", r#"echo "hello\nworld""#])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), r"hello\nworld");
}

#[test]
fn double_quote_with_dollar_expansion() {
    let out = Command::new(meiksh())
        .args(["-c", r#"x=val; echo "prefix${x}suffix""#])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "prefixvalsuffix"
    );
}

#[test]
fn empty_double_quote_parts() {
    let out = Command::new(meiksh())
        .args(["-c", "set -- \"\"; echo $#"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "1");
}

#[test]
fn braced_default_with_quoted_word() {
    let out = Command::new(meiksh())
        .args(["-c", r#"unset x; echo ${x:-"hello $(echo w)orld"}"#])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello world");
}

#[test]
fn braced_assign_with_expansion_word() {
    let out = Command::new(meiksh())
        .args(["-c", "unset x; echo ${x:=val$(echo ue)}; echo $x"])
        .output()
        .expect("run");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines[0], "value");
    assert_eq!(lines[1], "value");
}

#[test]
fn nested_braced_in_braced() {
    let out = Command::new(meiksh())
        .args(["-c", "y=inner; echo ${x:-${y}}"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "inner");
}

#[test]
fn nested_arith_with_parens() {
    let out = Command::new(meiksh())
        .args(["-c", "echo $((1+(2*3)))"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "7");
}

#[test]
fn nested_command_sub_in_braced() {
    let out = Command::new(meiksh())
        .args(["-c", "echo ${x:-$(echo nested)}"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "nested");
}

#[test]
fn tilde_trailing_slash_stripping() {
    let out = Command::new(meiksh())
        .args(["-c", "HOME=/root/; echo ~/foo"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "/root/foo");
}

#[test]
fn assignment_value_with_expansion() {
    let out = Command::new(meiksh())
        .args(["-c", "x=hello; y=\"${x} world\"; echo $y"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello world");
}

#[test]
fn command_sub_in_assignment_triggers_subshell() {
    let out = Command::new(meiksh())
        .args(["-c", "x=$(echo fromcmd); echo $x"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "fromcmd");
}

#[test]
fn glob_in_literal_part() {
    let td = TempDir::new("glob_lit");
    fs::write(td.path.join("a.txt"), "").expect("write");
    fs::write(td.path.join("b.txt"), "").expect("write");
    let script = format!(
        "cd {} && echo *.txt | tr ' ' '\\n' | sort",
        td.path.display()
    );
    let out = Command::new(meiksh())
        .args(["-c", &script])
        .output()
        .expect("run");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("a.txt") && stdout.contains("b.txt"));
}

#[test]
fn braced_error_default_message() {
    let out = Command::new(meiksh())
        .args(["-c", "unset x; echo ${x?} 2>&1"])
        .output()
        .expect("run");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("parameter null or not set"),
        "expected default error, got: {stderr}"
    );
}

#[test]
fn braced_default_no_colon_unset() {
    let out = Command::new(meiksh())
        .args(["-c", "unset x; echo ${x-fallback}"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "fallback");
}

#[test]
fn braced_default_no_colon_empty_keeps_empty() {
    let out = Command::new(meiksh())
        .args(["-c", "x=''; echo \"${x-fallback}end\""])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "end");
}

#[test]
fn braced_assign_no_colon() {
    let out = Command::new(meiksh())
        .args(["-c", "unset x; echo ${x=assigned}; echo $x"])
        .output()
        .expect("run");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines[0], "assigned");
    assert_eq!(lines[1], "assigned");
}

#[test]
fn at_empty_expansion_no_fields() {
    let out = Command::new(meiksh())
        .args(["-c", "set --; echo \"$@\"end"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "end");
}

#[test]
fn at_in_braced_default_produces_fields() {
    let out = Command::new(meiksh())
        .args([
            "-c",
            "set -- a b c; unset x; for w in ${x:-\"$@\"}; do echo \"$w\"; done",
        ])
        .output()
        .expect("run");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines, vec!["a", "b", "c"]);
}

#[test]
fn backslash_newline_continuation_in_word() {
    let out = Command::new(meiksh())
        .args(["-c", "echo hel\\\nlo"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello");
}

#[test]
fn backslash_escape_in_word() {
    let out = Command::new(meiksh())
        .args(["-c", r"echo hello\ world"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello world");
}

#[test]
fn quoted_tilde_stays_literal_via_parts() {
    let out = Command::new(meiksh())
        .args(["-c", "echo ~'user'"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "~user");
}

#[test]
fn braced_trim_with_expansion_pattern() {
    let out = Command::new(meiksh())
        .args(["-c", "pat='.*'; v=file.tar.gz; echo ${v%$pat}"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "file.tar");
}

#[test]
fn simple_var_downgrade_from_braces() {
    let out = Command::new(meiksh())
        .args(["-c", "x=val; echo ${x}"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "val");
}

#[test]
fn dollar_single_quote_control_backslash_c() {
    let out = Command::new(meiksh())
        .args(["-c", "printf '%s' $'\\cA' | od -An -tx1 | tr -d ' \\n'"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "01");
}

#[test]
fn shell_name_zero_via_parts() {
    let out = Command::new(meiksh())
        .args(["-c", "echo $0"])
        .output()
        .expect("run");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!stdout.trim().is_empty());
}

#[test]
fn nested_command_sub_with_quotes_in_parens() {
    let out = Command::new(meiksh())
        .args(["-c", "echo $(echo 'hello world')"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello world");
}

#[test]
fn braced_with_nested_arith_in_word() {
    let out = Command::new(meiksh())
        .args(["-c", "unset x; echo ${x:-$((1+2))}"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "3");
}

#[test]
fn braced_with_backtick_in_word() {
    let out = Command::new(meiksh())
        .args(["-c", "unset x; echo ${x:-`echo bt`}"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "bt");
}

#[test]
fn hash_at_start_not_expanded() {
    let out = Command::new(meiksh())
        .args(["-c", "echo ok # this is a comment"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "ok");
}

#[test]
fn braced_error_set_passes() {
    let out = Command::new(meiksh())
        .args(["-c", "x=ok; echo ${x?err}"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "ok");
}

#[test]
fn braced_error_colon_set_nonempty_passes() {
    let out = Command::new(meiksh())
        .args(["-c", "x=ok; echo ${x:?err}"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "ok");
}

#[test]
fn at_unquoted_expansion_via_parts() {
    let out = Command::new(meiksh())
        .args(["-c", "set -- hello world; echo $@"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello world");
}

#[test]
fn star_with_null_ifs() {
    let out = Command::new(meiksh())
        .args(["-c", "IFS=; set -- a b c; echo \"$*\""])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "abc");
}

#[test]
fn star_unset_ifs_defaults_to_space() {
    let out = Command::new(meiksh())
        .args(["-c", "unset IFS; set -- a b c; echo \"$*\""])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "a b c");
}

#[test]
fn braced_assign_colon_with_empty_value() {
    let out = Command::new(meiksh())
        .args(["-c", "x=''; echo ${x:=newval}; echo $x"])
        .output()
        .expect("run");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines[0], "newval");
    assert_eq!(lines[1], "newval");
}

#[test]
fn braced_assign_colon_with_set_value() {
    let out = Command::new(meiksh())
        .args(["-c", "x=existing; echo ${x:=unused}; echo $x"])
        .output()
        .expect("run");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines[0], "existing");
    assert_eq!(lines[1], "existing");
}

#[test]
fn multiline_literal_in_parts() {
    let out = Command::new(meiksh())
        .args(["-c", "printf '%s' 'line1\\nline2'"])
        .output()
        .expect("run");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("line1") && stdout.contains("line2"),
        "got: {stdout}"
    );
}

#[test]
fn trim_suffix_with_expansion_in_pattern() {
    let out = Command::new(meiksh())
        .args(["-c", "x=hello.world; pat='.world'; echo ${x%$pat}"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello");
}

#[test]
fn trim_suffix_no_match_returns_original() {
    let out = Command::new(meiksh())
        .args(["-c", "x=hello; echo ${x%.xyz}"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello");
}

#[test]
fn trim_prefix_no_match_returns_original() {
    let out = Command::new(meiksh())
        .args(["-c", "x=hello; echo ${x#xyz*}"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello");
}

#[test]
fn arithmetic_with_expansion_in_parts() {
    let out = Command::new(meiksh())
        .args(["-c", "x=10; echo $(($x + $(echo 5)))"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "15");
}

#[test]
fn glob_in_expanded_value_not_expanded_when_quoted() {
    let td = TempDir::new("glob_quoted");
    fs::write(td.path.join("a.txt"), "").expect("write");
    let script = format!("cd {} && x='*.txt'; echo \"$x\"", td.path.display());
    let out = Command::new(meiksh())
        .args(["-c", &script])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "*.txt");
}

#[test]
fn command_sub_in_prefix_assignment() {
    let out = Command::new(meiksh())
        .args(["-c", "x=$(echo val) env sh -c 'echo $x'"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "val");
}

#[test]
fn assignment_value_via_parts_path() {
    let out = Command::new(meiksh())
        .args(["-c", "x=$((2+3)); echo $x"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "5");
}

#[test]
fn tilde_user_with_trailing_slash_dir() {
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap();
    let out = Command::new(meiksh())
        .args(["-c", &format!("echo ~{user}/foo")])
        .output()
        .expect("run");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let trimmed = stdout.trim();
    assert!(
        trimmed.ends_with("/foo"),
        "expected /foo suffix, got: {trimmed}"
    );
    assert!(
        !trimmed.contains("//"),
        "should not have double slash, got: {trimmed}"
    );
}

#[test]
fn braced_with_dollar_in_word() {
    let out = Command::new(meiksh())
        .args(["-c", "y=sub; unset x; echo ${x:-pre${y}post}"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "presubpost");
}

#[test]
fn quoted_literal_with_newlines() {
    let out = Command::new(meiksh())
        .args(["-c", "echo \"line1\nline2\""])
        .output()
        .expect("run");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("line1\nline2"));
}

#[test]
fn expand_word_text_with_parts() {
    let out = Command::new(meiksh())
        .args(["-c", "x=hello; printf '%s\\n' \"$x world\""])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello world");
}

#[test]
fn tilde_followed_by_word_content() {
    let out = Command::new(meiksh())
        .args(["-c", "echo ~abc123def"])
        .output()
        .expect("run");
    assert!(out.status.success());
}

#[test]
fn tilde_alone_with_word_break() {
    let out = Command::new(meiksh())
        .args(["-c", "HOME=/home/test; echo ~ done"])
        .output()
        .expect("run");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("/home/test"));
    assert!(stdout.contains("done"));
}

#[test]
fn into_single_vec_with_ifs_split_fields() {
    let out = Command::new(meiksh())
        .args(["-c", "x='a b c'; y=\"${x}\"; echo $y"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "a b c");
}

#[test]
fn newlines_in_literal_part() {
    let out = Command::new(meiksh())
        .args(["-c", "x=abc\necho $x"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "abc");
}

#[test]
fn tilde_user_slash_expansion_via_parts() {
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap();
    let out = Command::new(meiksh())
        .args(["-c", &format!("echo ~{user}/test")])
        .output()
        .expect("run");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.trim().ends_with("/test"));
}

#[test]
fn expand_glob_in_unquoted_var() {
    let td = TempDir::new("glob_var");
    fs::write(td.path.join("x.txt"), "").expect("write");
    let script = format!("cd {} && g='*.txt'; echo $g", td.path.display());
    let out = Command::new(meiksh())
        .args(["-c", &script])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "x.txt");
}

#[test]
fn push_literal_glob_detection_via_star() {
    let td = TempDir::new("glob_star");
    fs::write(td.path.join("a.sh"), "").expect("write");
    fs::write(td.path.join("b.sh"), "").expect("write");
    let script = format!(
        "cd {} && echo *.sh | tr ' ' '\\n' | sort",
        td.path.display()
    );
    let out = Command::new(meiksh())
        .args(["-c", &script])
        .output()
        .expect("run");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("a.sh") && stdout.contains("b.sh"));
}

#[test]
fn at_single_param_finish_expansion() {
    let out = Command::new(meiksh())
        .args(["-c", "set -- only; echo \"$@\""])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "only");
}

#[test]
fn expand_word_text_via_parts() {
    let out = Command::new(meiksh())
        .args([
            "-c",
            "x='hello world'; [ \"$x\" = 'hello world' ] && echo ok",
        ])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "ok");
}

#[test]
fn expand_assignment_value_with_parts() {
    let out = Command::new(meiksh())
        .args(["-c", "x=$(echo val); echo $x"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "val");
}

#[test]
fn at_expansion_produces_fields_in_finish() {
    let out = Command::new(meiksh())
        .args(["-c", "set -- a b c; for x in \"$@\"; do echo $x; done"])
        .output()
        .expect("run");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines, vec!["a", "b", "c"]);
}

#[test]
fn parameter_plus_op_set_returns_word() {
    let out = Command::new(meiksh())
        .args(["-c", "x=''; echo \"${x+yes}\""])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "yes");
}

#[test]
fn parameter_plus_op_unset_returns_empty() {
    let out = Command::new(meiksh())
        .args(["-c", "unset x; echo \"${x+yes}end\""])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "end");
}

#[test]
fn parameter_colon_plus_op_empty_returns_empty() {
    let out = Command::new(meiksh())
        .args(["-c", "x=''; echo \"${x:+yes}end\""])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "end");
}

#[test]
fn expand_word_text_with_parts_and_expansion() {
    let out = Command::new(meiksh())
        .args([
            "-c",
            "x=world; case \"hello $x\" in 'hello world') echo match;; esac",
        ])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "match");
}

#[test]
fn assignment_with_braced_expansion() {
    let out = Command::new(meiksh())
        .args(["-c", "x=hello; y=\"${x} world\"; echo $y"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello world");
}

#[test]
fn tilde_home_trailing_slash_in_expand_raw() {
    let out = Command::new(meiksh())
        .args(["-c", "HOME=/root/; MYPATH=~/foo; echo $MYPATH"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "/root/foo");
}

#[test]
fn newlines_in_word_parts() {
    let out = Command::new(meiksh())
        .args(["-c", "echo 'line1\nline2' | wc -l"])
        .output()
        .expect("run");
    assert!(out.status.success());
}

#[test]
fn command_sub_in_arg_of_simple_command() {
    let out = Command::new(meiksh())
        .args(["-c", "echo $(echo hi) $(echo there)"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hi there");
}

#[test]
fn tilde_literal_ignored_in_arithmetic() {
    let out = Command::new(meiksh())
        .args(["-c", "echo $((~0))"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "-1");
}

#[test]
fn at_fields_in_braced_default() {
    let out = Command::new(meiksh())
        .args(["-c", "set -- a b c; unset x; printf '%s\\n' ${x:-\"$@\"}"])
        .output()
        .expect("run");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines, vec!["a", "b", "c"]);
}

#[test]
fn tilde_expansion_ignored_in_pattern_build() {
    let out = Command::new(meiksh())
        .args(["-c", "x='a~b'; echo ${x%'~'*}"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "a");
}

#[test]
fn push_literal_glob_via_tilde_fallback() {
    let out = Command::new(meiksh())
        .args(["-c", "echo ~nonexistentuser99999"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "~nonexistentuser99999"
    );
}

// ── Coverage: $'...' edge-case escapes (parameter.rs:204,222,225-226) ──

#[test]
fn dollar_single_quote_multi_escape_sequences() {
    let out = Command::new(meiksh())
        .args(["-c", r"printf '%s' $'\c\M' | od -An -tx1 | tr -d ' \n'"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "0d");
}

#[test]
fn dollar_single_quote_trailing_ctrl_c() {
    let out = Command::new(meiksh())
        .args(["-c", r"printf '%s' $'\c' | od -An -tx1 | tr -d ' \n'"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "");
}

#[test]
fn dollar_single_quote_ctrl_backslash_escape() {
    let out = Command::new(meiksh())
        .args(["-c", r"printf '%s' $'\c\\' | od -An -tx1 | tr -d ' \n'"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "1c");
}

// ── Coverage: ${x:+word} and ${x+word} returning empty (parameter.rs:595,599) ──

#[test]
fn colon_plus_unset_returns_empty() {
    let out = Command::new(meiksh())
        .args(["-c", "unset x; printf '<%s>' \"${x:+word}\""])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), "<>");
}

#[test]
fn plus_unset_returns_empty() {
    let out = Command::new(meiksh())
        .args(["-c", "unset x; printf '<%s>' \"${x+word}\""])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), "<>");
}

// ── Coverage: AtEmpty in parameter word (parameter.rs:822) ──

#[test]
fn at_empty_in_braced_default_word() {
    let out = Command::new(meiksh())
        .args(["-c", r#"set --; unset x; printf '<%s>' "${x:-$@}""#])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), "<>");
}

// ── Coverage: IFS-empty disables splitting (word.rs:214) ──

#[test]
fn empty_ifs_disables_splitting() {
    let out = Command::new(meiksh())
        .args(["-c", "IFS=''; x='a b c'; printf '<%s>' $x"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), "<a b c>");
}

// ── Coverage: Expansion::Static in arithmetic (arithmetic.rs:20) ──

#[test]
fn arithmetic_with_special_param_static() {
    let out = Command::new(meiksh())
        .args(["-c", "echo $(( $? + 1 ))"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "1");
}

// ── Coverage: Expansion::Static in flatten_expansion (word.rs:455) ──

#[test]
fn special_param_in_redirect_target() {
    let td = TempDir::new("redir_sp");
    let script = format!("cd {} && echo hi > \"$?\" && cat 0", td.path.display());
    let out = Command::new(meiksh())
        .args(["-c", &script])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hi");
}

// ── Coverage: redirect word expanding to multiple fields (word.rs:258) ──

#[test]
fn redirect_word_joins_multiple_fields() {
    let td = TempDir::new("redir_mf");
    let script = format!(
        "cd {} && x='a b'; echo hi > \"$x\"; cat 'a b'",
        td.path.display()
    );
    let out = Command::new(meiksh())
        .args(["-c", &script])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hi");
}

// ── Coverage: assignment value via word-parts path (word.rs:308-317) ──

#[test]
fn assignment_value_via_word_parts_at_expansion() {
    let out = Command::new(meiksh())
        .args(["-c", "set -- x y z; v=$@; echo \"$v\""])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "x y z");
}

// ── Coverage: drain_single_vec with non-empty fields (expand_parts.rs:117-127) ──

#[test]
fn drain_single_vec_with_star_in_assignment() {
    let out = Command::new(meiksh())
        .args(["-c", "IFS=:; set -- a b c; x=$*; echo \"$x\""])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "a:b:c");
}

// ── Coverage: tilde HOME empty → empty field (word.rs:652-654) ──

#[test]
fn tilde_home_empty_raw_path() {
    let out = Command::new(meiksh())
        .args(["-c", "HOME=''; printf '<%s>' ~"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), "<>");
}

// ── Coverage: tilde HOME unset → literal ~ (word.rs:655-657) ──

#[test]
fn tilde_home_unset_raw_path() {
    let out = Command::new(meiksh())
        .args(["-c", "unset HOME; printf '<%s>' ~"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), "<~>");
}

// ── Coverage: has_command_substitution word-parts path (exec/simple.rs:126-128,134) ──

#[test]
fn has_command_sub_in_prefix_assignment_triggers_fork() {
    let out = Command::new(meiksh())
        .args(["-c", "x=$(echo val) printenv x"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "val");
}

// ── Coverage: backslash at EOF in raw word (token.rs:1275-1278) ──

#[test]
fn heredoc_delimiter_with_hash_start() {
    let out = Command::new(meiksh())
        .args(["-c", "cat <<\\#END\nhello\n#END"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello");
}

// ── Coverage: line continuation at word start → delimiter (token.rs:1261-1266) ──

#[test]
fn heredoc_delimiter_with_dollar_construct() {
    let out = Command::new(meiksh())
        .args(["-c", "cat <<$EOF\nhello\n$EOF"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello");
}

// ── Coverage: literal dollar at end of input (token.rs:1692-1697) ──

#[test]
fn literal_dollar_at_end_of_word() {
    let out = Command::new(meiksh())
        .args(["-c", "echo \"hello$\""])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello$");
}

// ── Coverage: backtick in raw word scanning (token.rs:1308-1311) ──

#[test]
fn backtick_in_raw_word_scan() {
    let out = Command::new(meiksh())
        .args(["-c", "echo pre`echo mid`post"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "premidpost");
}

// ── Coverage: unterminated backtick in dquotes (token.rs:1671,1874) ──

#[test]
fn unterminated_backtick_in_dquotes() {
    let out = Command::new(meiksh())
        .args(["-c", "echo \"`echo hi`\""])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hi");
}

// ── Coverage: ${HOME} simplifies to SimpleVar (token.rs:1762) ──

#[test]
fn braced_var_simplifies_to_simple_var() {
    let out = Command::new(meiksh())
        .args(["-c", "x=test; echo ${x}end"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "testend");
}

// ── Coverage: $((literal)) → ArithmeticLiteral (token.rs:1791) ──

#[test]
fn arithmetic_literal_optimization() {
    let out = Command::new(meiksh())
        .args(["-c", "echo $((100))"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "100");
}

// ── Coverage: tilde user/path + word-break in parts (token.rs:1389,1404,1407,1419) ──

#[test]
fn tilde_user_slash_path_then_word_break() {
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap();
    let out = Command::new(meiksh())
        .args(["-c", &format!("echo ~{user}/path rest")])
        .output()
        .expect("run");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parts: Vec<&str> = stdout.trim().split_whitespace().collect();
    assert_eq!(parts.len(), 2);
    assert!(parts[0].ends_with("/path"));
    assert_eq!(parts[1], "rest");
}

#[test]
fn bare_tilde_at_end_of_input() {
    let out = Command::new(meiksh())
        .args(["-c", "HOME=/testhome; echo ~"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "/testhome");
}

// ── Coverage: tilde in braced word (token.rs:1995-1996, expand_parts.rs:601,630) ──

#[test]
fn tilde_in_braced_default_word() {
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap();
    let out = Command::new(meiksh())
        .args(["-c", &format!("unset x; echo ${{x:-~{user}/stuff}}")])
        .output()
        .expect("run");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.trim().ends_with("/stuff"),
        "expected tilde expansion: {stdout}"
    );
}

// ── Coverage: non-special backslash in dquotes in word parts (token.rs:2042-2045) ──

#[test]
fn non_special_backslash_in_dquotes_braced_word() {
    let out = Command::new(meiksh())
        .args(["-c", r#"unset x; echo ${x:-"hello\wworld"}"#])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), r"hello\wworld");
}

// ── Coverage: backslash escape in unquoted braced word (token.rs:2084) ──

#[test]
fn backslash_escape_in_unquoted_braced_word() {
    let out = Command::new(meiksh())
        .args(["-c", r"unset x; echo ${x:-hello\ world}"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello world");
}

// ── Coverage: quoted expansion in pattern (expand_parts.rs:595) ──

#[test]
fn quoted_expansion_in_trim_pattern() {
    let out = Command::new(meiksh())
        .args(["-c", r#"p='.tar'; x=file.tar.gz; echo "${x%"$p".*}""#])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "file");
}

// ── Coverage: newlines in literal word part (expand_parts.rs:228-229) ──

#[test]
fn newlines_in_literal_word_part_inc_lineno() {
    let out = Command::new(meiksh())
        .args(["-c", "eval 'x=1\ny=2'; echo $x $y"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "1 2");
}

// ── Coverage: push_literal with glob char (expand_parts.rs:50) ──

#[test]
fn push_literal_glob_char_in_expansion() {
    let td = TempDir::new("plglob");
    fs::write(td.path.join("a.txt"), "").expect("write");
    let script = format!("cd {} && unset x; echo ${{x:-*.txt}}", td.path.display());
    let out = Command::new(meiksh())
        .args(["-c", &script])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "a.txt");
}

// ── Coverage: tilde with trailing-slash home in expand_parts (expand_parts.rs:284) ──

#[test]
fn tilde_trailing_slash_home_in_braced_word() {
    let out = Command::new(meiksh())
        .args(["-c", "HOME=/home/test/; unset x; echo ${x:-~/foo}"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "/home/test/foo"
    );
}

// ── Coverage: flush_literal merging (token.rs:1902,1905) ──

#[test]
fn flush_literal_merges_adjacent_spans() {
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap();
    let out = Command::new(meiksh())
        .args(["-c", &format!("echo ~{user}/a/b/c")])
        .output()
        .expect("run");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.trim().ends_with("/a/b/c"));
}

// ── Coverage: AsBytes for Vec<u8> (expand_parts.rs:680-682) ──

#[test]
fn as_bytes_vec_used_in_expansion() {
    let out = Command::new(meiksh())
        .args(["-c", r#"x="hello world"; echo "${x#hell}""#])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "o world");
}

// ── Coverage: word-parts path non-at-break (word.rs:160) + empty text (word.rs:193) ──
// These are dead/unreachable code paths in the raw fallback expansion.
// word.rs:160 - within has_at_expansion but without AtBreak or AtEmpty (logically impossible)
// word.rs:193 - raw expansion producing empty non-expanded text

// ── Coverage: tilde user home dir trailing slash (word.rs:662) ──

#[test]
fn tilde_user_trailing_slash_stripped_in_raw() {
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap();
    let out = Command::new(meiksh())
        .args(["-c", &format!("v=~{user}/sub; echo $v")])
        .output()
        .expect("run");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.trim().ends_with("/sub"),
        "expected /sub suffix: {stdout}"
    );
    assert!(!stdout.trim().contains("//"), "no double slash: {stdout}");
}

// ── Coverage: # at start of raw word (token.rs:1254) ──

#[test]
fn hash_at_raw_word_start_is_comment() {
    let out = Command::new(meiksh())
        .args(["-c", "echo ok; #echo nope"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "ok");
}

// ── Coverage: $ in raw word scanning (token.rs:1303) ──

#[test]
fn dollar_construct_in_raw_word() {
    let out = Command::new(meiksh())
        .args(["-c", "x=val; echo pre${x}post"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "prevalpost");
}

// ── Coverage: word_parts non-empty resume (token.rs:1192) ──

#[test]
fn word_parts_resume_after_tilde() {
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap();
    let out = Command::new(meiksh())
        .args(["-c", &format!("echo ~{user}/dir${{x:-/sub}}")])
        .output()
        .expect("run");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.trim().ends_with("/dir/sub"));
}

// ── Coverage: heredoc with $ in delimiter (scan_raw_word token.rs:1303) ──

#[test]
fn heredoc_dollar_in_delimiter() {
    use std::io::Write;
    use std::process::Stdio;
    let mut child = Command::new(meiksh())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"cat <<$END\nhello\n$END\n")
        .unwrap();
    let out = child.wait_with_output().expect("wait");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello");
}

// ── Coverage: heredoc with backtick in delimiter (scan_raw_word token.rs:1308-1311) ──

#[test]
fn heredoc_backtick_in_delimiter() {
    use std::io::Write;
    use std::process::Stdio;
    let mut child = Command::new(meiksh())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"cat <<\\`END\\`\nhello\n`END`\n")
        .unwrap();
    let out = child.wait_with_output().expect("wait");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello");
}

// ── Coverage: heredoc with backslash-newline at start (scan_raw_word token.rs:1261-1266) ──

#[test]
fn heredoc_backslash_newline_at_start_of_delim() {
    let out = Command::new(meiksh())
        .args(["-c", "cat <<\\\nEOF\nhello\nEOF"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello");
}

#[test]
fn heredoc_backslash_newline_then_delimiter_break() {
    let out = Command::new(meiksh())
        .args(["-c", "cat <<\\\n;\nhello\n;"])
        .output()
        .expect("run");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("expected heredoc delimiter"));
}

// ── Coverage: heredoc delimiter with trailing backslash at EOF (token.rs:1275-1278) ──

#[test]
fn heredoc_trailing_backslash_in_delimiter() {
    let out = Command::new(meiksh())
        .args(["-c", "cat <<END\\"])
        .output()
        .expect("run");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("unterminated here-document"));
}

// ── Coverage: literal $ at end of word in dquote word-parts (token.rs:1692-1697) ──

#[test]
fn literal_dollar_at_end_of_dquoted_word() {
    let out = Command::new(meiksh())
        .args(["-c", "echo \"a$\""])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "a$");
}

// ── Coverage: unterminated backtick in dquoted word (token.rs:1671) ──
// Note: `find_backtick_end_in_slice` returning raw.len() (line 1671/1874)
// is a defensive path — the tokenizer rejects unterminated backticks before
// the word-parts builder runs. This path is effectively unreachable.

// ── Coverage: # at heredoc delimiter start (token.rs:1254) ──

#[test]
fn heredoc_delimiter_is_hash_comment() {
    let out = Command::new(meiksh())
        .args(["-c", "cat <<\\#EOF\nhello\n#EOF"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello");
}

#[test]
fn heredoc_hash_at_delimiter_start_is_error() {
    let out = Command::new(meiksh())
        .args(["-c", "cat << #comment"])
        .output()
        .expect("run");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("expected heredoc delimiter"));
}

// ── Coverage: backslash at EOF in heredoc delimiter (via -c) ──

#[test]
fn heredoc_delimiter_has_backslash_escaped_char() {
    let out = Command::new(meiksh())
        .args(["-c", "cat <<E\\ND\nhello\nEND"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello");
}
