use super::common::*;
use std::fs;
use std::process::Command;

#[test]
fn handles_redirections() {
    let root = TempDir::new("meiksh-redir");
    let path = root.join("out.txt");
    let script = format!("printf hi > {}", path.display());

    let output = Command::new(meiksh())
        .args(["-c", &script])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(
        fs::read_to_string(&path).expect("read redirect target"),
        "hi"
    );
}

#[test]
fn redirects_current_shell_builtins_and_compound_commands() {
    let root = TempDir::new("meiksh-builtin-redir");
    let builtin_path = root.join("builtin.txt");
    let group_path = root.join("group.txt");

    let builtin = Command::new(meiksh())
        .args([
            "-c",
            &format!("pwd > {}; printf ok", builtin_path.display()),
        ])
        .output()
        .expect("run meiksh");
    assert!(builtin.status.success());
    assert_eq!(String::from_utf8_lossy(&builtin.stdout), "ok");
    assert!(
        !fs::read_to_string(&builtin_path)
            .expect("read builtin output")
            .trim()
            .is_empty()
    );

    let group = Command::new(meiksh())
        .args([
            "-c",
            &format!(
                "{{ printf inside; }} > {}; printf outside",
                group_path.display()
            ),
        ])
        .output()
        .expect("run meiksh");
    assert!(group.status.success());
    assert_eq!(String::from_utf8_lossy(&group.stdout), "outside");
    assert_eq!(
        fs::read_to_string(&group_path).expect("read group output"),
        "inside"
    );

    let pipeline = Command::new(meiksh())
        .args([
            "-c",
            &format!("{{ printf inside; }} > {} | wc -c", group_path.display()),
        ])
        .output()
        .expect("run meiksh");
    assert!(pipeline.status.success());
    assert_eq!(String::from_utf8_lossy(&pipeline.stdout).trim(), "0");
    assert_eq!(
        fs::read_to_string(&group_path).expect("read group output again"),
        "inside"
    );
}

#[test]
fn handles_append_and_input_redirections() {
    let root = TempDir::new("meiksh-append-input");
    let input = root.join("input.txt");
    let output = root.join("output.txt");
    fs::write(&input, "abc").expect("write input");

    let script = format!(
        "cat < {} > {}; printf def >> {}",
        input.display(),
        output.display(),
        output.display()
    );
    let status = Command::new(meiksh())
        .args(["-c", &script])
        .status()
        .expect("run meiksh");
    assert!(status.success());
    assert_eq!(fs::read_to_string(&output).expect("read output"), "abcdef");
}

#[test]
fn handles_extended_redirection_matrix() {
    let dir = TempDir::new("meiksh-redir-matrix");

    let input = dir.join("input.txt");
    let output = dir.join("output.txt");
    let append = dir.join("append.txt");
    let rw = dir.join("rw.txt");
    fs::write(&input, "from-input").expect("write input");
    fs::write(&rw, "from-rw").expect("write rw");

    let read_script = format!("cat 3<{} <&3", input.display());
    let read_output = Command::new(meiksh())
        .args(["-c", &read_script])
        .output()
        .expect("run meiksh");
    assert!(read_output.status.success());
    assert_eq!(String::from_utf8_lossy(&read_output.stdout), "from-input");

    let write_script = format!("printf file 3>{} >&3", output.display());
    let write_status = Command::new(meiksh())
        .args(["-c", &write_script])
        .status()
        .expect("run meiksh");
    assert!(write_status.success());
    assert_eq!(fs::read_to_string(&output).expect("read output"), "file");

    let append_script = format!("printf err 2>>{} >&2", append.display());
    let append_status = Command::new(meiksh())
        .args(["-c", &append_script])
        .status()
        .expect("run meiksh");
    assert!(append_status.success());
    assert_eq!(fs::read_to_string(&append).expect("read append"), "err");

    let read_write_script = format!("cat <>{}", rw.display());
    let rw_output = Command::new(meiksh())
        .args(["-c", &read_write_script])
        .output()
        .expect("run meiksh");
    assert!(rw_output.status.success());
    assert_eq!(String::from_utf8_lossy(&rw_output.stdout), "from-rw");

    let precedence_script = format!("printf hidden 1>{} | wc -c", output.display());
    let precedence_output = Command::new(meiksh())
        .args(["-c", &precedence_script])
        .output()
        .expect("run meiksh");
    assert!(precedence_output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&precedence_output.stdout).trim(),
        "0"
    );
    assert_eq!(
        fs::read_to_string(&output).expect("read redirected output"),
        "hidden"
    );
}

#[test]
fn honors_noclobber_and_force_clobber() {
    let root = TempDir::new("meiksh-noclobber");
    let path = root.join("test.txt");
    fs::write(&path, "old").expect("write initial");

    let blocked = Command::new(meiksh())
        .args(["-c", &format!("set -C; printf new > {}", path.display())])
        .output()
        .expect("run meiksh");
    assert!(!blocked.status.success());
    assert_eq!(fs::read_to_string(&path).expect("read blocked"), "old");

    let forced = Command::new(meiksh())
        .args(["-c", &format!("set -C; printf new >| {}", path.display())])
        .output()
        .expect("run meiksh");
    assert!(forced.status.success());
    assert_eq!(fs::read_to_string(&path).expect("read forced"), "new");
}

#[test]
fn redirect_word_not_globbed() {
    let tmp = TempDir::new("redirect-glob");
    let out = Command::new(meiksh())
        .args([
            "-c",
            &format!(
                "cd {d}; touch a_1.txt a_2.txt; echo literal > a_*.txt; cat a_\\*.txt",
                d = tmp.path.display()
            ),
        ])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "literal");
}

#[test]
fn redirect_word_with_unquoted_expansion() {
    let tmp = TempDir::new("redirect-expand");
    let out = Command::new(meiksh())
        .args([
            "-c",
            &format!(
                "x={d}/redir_out.txt; echo content > $x; cat {d}/redir_out.txt",
                d = tmp.path.display()
            ),
        ])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "content");
}

#[test]
fn fd_close_via_exec_and_child_error() {
    let tmp = TempDir::new("fd-close");
    let script = format!(
        "exec 4>{d}/out.txt; echo ok >&4; exec 4>&-; echo fail >&4",
        d = tmp.path.display()
    );
    let out = Command::new(meiksh())
        .args(["-c", &script])
        .output()
        .expect("run");
    assert_ne!(out.status.code(), Some(0));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("Bad file descriptor"), "stderr: {stderr}");
}

#[test]
fn blank_between_fd_and_redirect_operator() {
    let tmp = TempDir::new("blank-fd");
    let script = format!(
        "echo literal 2 > {d}/out.txt; cat {d}/out.txt",
        d = tmp.path.display()
    );
    let out = Command::new(meiksh())
        .args(["-c", &script])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "literal 2");
}

// ── Here documents ──

#[test]
fn executes_here_documents() {
    let output = Command::new(meiksh())
        .args(["-c", "VALUE=world; cat <<EOF\nhello $VALUE\nEOF\n"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "hello world\n");

    let quoted = Command::new(meiksh())
        .args(["-c", "VALUE=world; cat <<'EOF'\nhello $VALUE\nEOF\n"])
        .output()
        .expect("run meiksh");
    assert!(quoted.status.success());
    assert_eq!(String::from_utf8_lossy(&quoted.stdout), "hello $VALUE\n");

    let stripped = Command::new(meiksh())
        .args(["-c", "cat <<-\tEOF\n\tstrip-me\n\tEOF\n"])
        .output()
        .expect("run meiksh");
    assert!(stripped.status.success());
    assert_eq!(String::from_utf8_lossy(&stripped.stdout), "strip-me\n");
}

#[test]
fn here_document_backtick_expansion() {
    let out = Command::new(meiksh())
        .args(["-c", "cat <<EOF\n`echo hello`\nEOF\n"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello");
}

#[test]
fn here_document_backslash_newline_continuation() {
    let out = Command::new(meiksh())
        .args(["-c", "cat <<EOF\nEO\\\nF\necho after\n"])
        .output()
        .expect("run");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("after"), "stdout: {stdout}");
}

#[test]
fn heredoc_escaped_backslash_not_continuation() {
    let out = Command::new(meiksh())
        .args(["-c", "cat <<EOF\n\\$val\n\\\\\nEOF\n"])
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(stdout.trim(), "$val\n\\");
}

#[test]
fn heredoc_strip_tabs_after_continuation() {
    let out = Command::new(meiksh())
        .args(["-c", "cat <<-DELIM\n\tcont\\\n\tinued\nDELIM\n"])
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(stdout.trim(), "cont\tinued");
}

#[test]
fn heredoc_with_quoted_delimiter() {
    let out = Command::new(meiksh())
        .args(["-c", "cat <<'END'\nhello $HOME\nEND"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello $HOME");
}

#[test]
fn heredoc_with_backslash_in_delimiter() {
    let out = Command::new(meiksh())
        .args(["-c", "cat <<E\\ND\nhello\nEND"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello");
}

#[test]
fn heredoc_with_dollar_construct_in_body() {
    let out = Command::new(meiksh())
        .args(["-c", "x=world; cat <<EOF\nhello $x\nEOF"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello world");
}

#[test]
fn heredoc_with_dollar_single_quote() {
    let out = Command::new(meiksh())
        .args(["-c", "cat <<$'EOF'\nhello\nEOF"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello");
}

#[test]
fn heredoc_with_double_digit_delimiter() {
    let out = Command::new(meiksh())
        .args(["-c", "cat <<12\nhello\n12"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello");
}

#[test]
fn heredoc_with_double_quote_delimiter() {
    let out = Command::new(meiksh())
        .args(["-c", "cat <<\"EOF\"\nhello\nEOF"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello");
}

#[test]
fn heredoc_with_single_quote_delim() {
    let out = Command::new(meiksh())
        .args(["-c", "cat <<'E N D'\nhello $HOME\nE N D"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello $HOME");
}

#[test]
fn heredoc_backslash_newline_continuation() {
    let out = Command::new(meiksh())
        .args(["-c", "cat <<\\\nEOF\nhello\nEOF"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello");
}

#[test]
fn heredoc_plain_word() {
    let out = Command::new(meiksh())
        .args(["-c", "cat <<MYEOF\nhello\nMYEOF"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello");
}

#[test]
fn double_heredoc() {
    let out = Command::new(meiksh())
        .args(["-c", "cat <<A; cat <<B\nfirst\nA\nsecond\nB"])
        .output()
        .expect("run");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("first") && stdout.contains("second"));
}

// ── Redirect expansion coverage ──

#[test]
fn redirect_word_with_expansion() {
    let td = TempDir::new("redir_expand");
    let script = format!(
        "x=outfile; echo hello > {}/\"$x\"; cat {}/outfile",
        td.path.display(),
        td.path.display()
    );
    let out = Command::new(meiksh())
        .args(["-c", &script])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello");
}

#[test]
fn redirect_with_variable_in_filename() {
    let td = TempDir::new("redir_var");
    let script = format!(
        "d={}; echo hello > \"$d/out.txt\"; cat \"$d/out.txt\"",
        td.path.display()
    );
    let out = Command::new(meiksh())
        .args(["-c", &script])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello");
}

#[test]
fn expand_redirect_with_parts() {
    let td = TempDir::new("redir_parts");
    let script = format!(
        "x=test; echo data > {}/\"$x\".out; cat {}/test.out",
        td.path.display(),
        td.path.display()
    );
    let out = Command::new(meiksh())
        .args(["-c", &script])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "data");
}

#[test]
fn redirect_with_glob_expansion() {
    let td = TempDir::new("redir_glob");
    fs::write(td.path.join("target.txt"), "").expect("write");
    let script = format!(
        "echo data > {}/target.txt; cat {}/target.txt",
        td.path.display(),
        td.path.display()
    );
    let out = Command::new(meiksh())
        .args(["-c", &script])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "data");
}
