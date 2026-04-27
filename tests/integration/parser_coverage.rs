use super::common::meiksh;
use std::process::Command;

#[test]
fn find_closing_paren_with_quotes() {
    let out = Command::new(meiksh())
        .args(["-c", "echo $(echo \"(not a paren)\")"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "(not a paren)");
}

#[test]
fn find_closing_brace_with_nested_arith() {
    let out = Command::new(meiksh())
        .args(["-c", "x=5; echo ${x:-$((1+2))}"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "5");
}

#[test]
fn find_closing_brace_with_nested_cmd_sub() {
    let out = Command::new(meiksh())
        .args(["-c", "unset x; echo ${x:-$(echo nested)}"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "nested");
}

#[test]
fn find_closing_brace_with_nested_brace() {
    let out = Command::new(meiksh())
        .args(["-c", "y=inner; unset x; echo ${x:-${y:-default}}"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "inner");
}

#[test]
fn arith_with_single_quotes() {
    let out = Command::new(meiksh())
        .args(["-c", "echo $(( '1' + '2' ))"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "3");
}

#[test]
fn arith_with_double_quotes() {
    let out = Command::new(meiksh())
        .args(["-c", "x=5; echo $(( \"$x\" + 1 ))"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "6");
}

#[test]
fn braced_with_backslash_in_word() {
    let out = Command::new(meiksh())
        .args(["-c", r#"unset x; echo "${x:-hello\ world}""#])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello world");
}

#[test]
fn braced_with_single_quote_in_word() {
    let out = Command::new(meiksh())
        .args(["-c", "unset x; echo ${x:-'literal text'}"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "literal text");
}

#[test]
fn find_closing_brace_with_quotes() {
    let out = Command::new(meiksh())
        .args(["-c", r#"unset x; echo ${x:-"}"}"#])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "}");
}

#[test]
fn find_closing_brace_with_single_quotes() {
    let out = Command::new(meiksh())
        .args(["-c", "unset x; echo ${x:-'}'}"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "}");
}

#[test]
fn find_closing_brace_with_escape() {
    let out = Command::new(meiksh())
        .args(["-c", r#"unset x; echo "${x:-\}ok}""#])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "}ok");
}

#[test]
fn double_quote_backslash_special_chars() {
    let out = Command::new(meiksh())
        .args(["-c", r#"echo "a\$b\\c\`d\"e""#])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "a$b\\c`d\"e");
}

#[test]
fn dollar_single_quote_unknown_escape() {
    let out = Command::new(meiksh())
        .args(["-c", "printf '%s' $'\\z'"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), "z");
}

#[test]
fn dollar_single_quote_hex_no_digits() {
    let out = Command::new(meiksh())
        .args(["-c", "printf '%s' $'\\xZZ'"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), "xZZ");
}

#[test]
fn arith_in_find_closing_brace() {
    let out = Command::new(meiksh())
        .args(["-c", "x=5; echo ${x:-$((2+3))}"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "5");
}

#[test]
fn double_quote_with_backtick_in_braced_word() {
    let out = Command::new(meiksh())
        .args(["-c", "unset x; echo \"${x:-`echo hi`}\""])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hi");
}

#[test]
fn double_quote_backslash_non_special() {
    let out = Command::new(meiksh())
        .args(["-c", r#"echo "\a\b\z""#])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), r"\a\b\z");
}

#[test]
fn braced_with_double_quote_in_word() {
    let out = Command::new(meiksh())
        .args(["-c", "unset x; echo ${x:-\"hello world\"}"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello world");
}

#[test]
fn braced_with_backslash_escape_in_word() {
    let out = Command::new(meiksh())
        .args(["-c", "unset x; echo \"${x:-a\\\\b}\""])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "a\\b");
}

#[test]
fn braced_with_nested_command_sub_in_braced() {
    let out = Command::new(meiksh())
        .args(["-c", "unset x; echo ${x:-$(echo ${y:-deep})}"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "deep");
}

#[test]
fn backtick_in_braced_word_of_parts() {
    let out = Command::new(meiksh())
        .args(["-c", "unset x; echo \"${x:-`echo tick`}\""])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "tick");
}

#[test]
fn dollar_single_quote_octal_escape() {
    let out = Command::new(meiksh())
        .args(["-c", "printf '%s' $'\\101\\102\\103'"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), "ABC");
}

#[test]
fn find_paren_with_escaped_paren() {
    let out = Command::new(meiksh())
        .args(["-c", "echo $(echo '()')"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "()");
}

#[test]
fn backslash_newline_at_start_of_word() {
    let out = Command::new(meiksh())
        .args(["-c", "echo \\\nhello"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello");
}

#[test]
fn braced_arith_and_cmd_sub_in_find_brace() {
    let out = Command::new(meiksh())
        .args(["-c", "unset x; echo ${x:-$((1+2))$(echo ok)}"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "3ok");
}

#[test]
fn paren_with_double_quotes_in_cmd_sub() {
    let out = Command::new(meiksh())
        .args(["-c", "echo $(echo \"hi()\")"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hi()");
}

#[test]
fn paren_with_backslash_in_cmd_sub() {
    let out = Command::new(meiksh())
        .args(["-c", "echo $(echo a\\)b)"])
        .output()
        .expect("run");
    assert!(out.status.success());
}

#[test]
fn arith_with_nested_parens_in_find_arith() {
    let out = Command::new(meiksh())
        .args(["-c", "echo $((2 * (3 + (4 - 1))))"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "12");
}

#[test]
fn braced_positional_large_index() {
    let out = Command::new(meiksh())
        .args(["-c", "echo ${100:-none}"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "none");
}

#[test]
fn arith_with_double_quotes_in_find_arith() {
    let out = Command::new(meiksh())
        .args(["-c", "x=5; echo $((\"$x\" + 1))"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "6");
}

#[test]
fn braced_word_dquote_with_dollar_var() {
    let out = Command::new(meiksh())
        .args(["-c", "y=val; unset x; echo ${x:-\"pre$y post\"}"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "preval post");
}

#[test]
fn braced_word_dquote_with_backtick() {
    let out = Command::new(meiksh())
        .args(["-c", "unset x; echo ${x:-\"pre`echo bt`post\"}"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "prebtpost");
}

#[test]
fn braced_word_dquote_with_backslash_dollar() {
    let out = Command::new(meiksh())
        .args(["-c", "unset x; echo \"${x:-a\\$b}\""])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "a$b");
}

#[test]
fn braced_word_dquote_with_backslash_backtick() {
    let out = Command::new(meiksh())
        .args(["-c", "unset x; echo \"${x:-a\\`echo b\\`c}\""])
        .output()
        .expect("run");
    assert!(out.status.success());
}

#[test]
fn braced_word_top_level_backslash() {
    let out = Command::new(meiksh())
        .args(["-c", "unset x; echo \"${x:-a\\nb}\""])
        .output()
        .expect("run");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("a") && stdout.contains("b"));
}

#[test]
fn braced_word_top_level_dollar() {
    let out = Command::new(meiksh())
        .args(["-c", "y=inner; unset x; echo ${x:-$y}"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "inner");
}

#[test]
fn braced_word_top_level_backtick() {
    let out = Command::new(meiksh())
        .args(["-c", "unset x; echo ${x:-`echo back`}"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "back");
}

#[test]
fn scan_raw_word_hash_comment() {
    let out = Command::new(meiksh())
        .args(["-c", "cat <<END\n# not a comment\nEND"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "# not a comment"
    );
}

#[test]
fn scan_raw_word_backslash_eof() {
    let out = Command::new(meiksh())
        .args(["-c", "echo hello\\"])
        .output()
        .expect("run");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("hello"));
}

#[test]
fn scan_raw_word_dollar_construct() {
    let out = Command::new(meiksh())
        .args(["-c", "x=val; cat <<END\n${x}\nEND"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "val");
}

#[test]
fn build_dollar_parts_short_dollar() {
    let out = Command::new(meiksh())
        .args(["-c", "echo \"$ end\""])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "$ end");
}

#[test]
fn classify_dollar_empty_braces() {
    let out = Command::new(meiksh())
        .args(["-c", "echo ${}"])
        .output()
        .expect("run");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("bad substitution"), "stderr: {stderr}");
}

#[test]
fn classify_braced_name_digit_overflow() {
    let out = Command::new(meiksh())
        .args(["-c", "echo ${99999999999999999999:-overflow}"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "overflow");
}

#[test]
fn classify_braced_op_all_ops() {
    let out = Command::new(meiksh())
        .args([
            "-c",
            "v=abcabc; echo \"[${v%%a*}][${v%a*}][${v##*a}][${v#*a}]\"",
        ])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "[][abc][bc][bcabc]"
    );
}

#[test]
fn classify_braced_op_colon_ops() {
    let out = Command::new(meiksh())
        .args([
            "-c",
            "unset a; a=''; echo ${a:=v1}; echo ${a:?err}; echo ${a:+alt}; b=''; echo ${b:-def}",
        ])
        .output()
        .expect("run");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines[0], "v1");
    assert_eq!(lines[1], "v1");
    assert_eq!(lines[2], "alt");
    assert_eq!(lines[3], "def");
}

#[test]
fn parse_braced_name_special_chars() {
    let out = Command::new(meiksh())
        .args(["-c", "set -- a b; echo ${!} ${$} ${?} ${*} ${@}"])
        .output()
        .expect("run");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parts: Vec<&str> = stdout.trim().split_whitespace().collect();
    assert!(parts.len() >= 3);
}

#[test]
fn scan_raw_word_backslash_newline_at_start() {
    let out = Command::new(meiksh())
        .args(["-c", "cat <<\\\nEOF\nhello\nEOF"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello");
}

#[test]
fn scan_raw_word_backslash_escape() {
    let out = Command::new(meiksh())
        .args(["-c", "cat <<E\\ F\nhello\nE F"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello");
}
