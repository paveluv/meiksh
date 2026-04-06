#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Requirement {
    pub id: String,
    pub doc: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct TestCase {
    pub name: String,
    pub interactive: bool,
    pub line_num: usize,
    pub requirements: Vec<Requirement>,
    pub env_overrides: Vec<(String, String)>,
    pub script_lines: Vec<(usize, String)>,
    pub script: Option<String>,
    pub expect_stdout: Option<(usize, String)>,
    pub expect_stderr: Option<(usize, String)>,
    pub expect_exit_code: Option<(usize, String)>,
}

#[derive(Debug, Clone)]
pub struct TestSuite {
    pub name: String,
    pub filename: String,
    pub tests: Vec<TestCase>,
}

pub fn parse_suite(text: &str, filename: &str) -> Result<TestSuite, String> {
    {
        let mut inside_script = false;
        for (lineno, raw_line) in text.lines().enumerate() {
            if inside_script {
                if raw_line == "  expect" {
                    inside_script = false;
                }
                continue;
            }
            if raw_line.trim() == "script" && raw_line.starts_with("  ") {
                inside_script = true;
                continue;
            }
            if raw_line.ends_with(' ') || raw_line.ends_with('\t') {
                return Err(format!(
                    "line {}: trailing whitespace: {:?}",
                    lineno + 1,
                    raw_line
                ));
            }
            if raw_line.contains("{{SHELL}}") {
                return Err(format!(
                    "line {}: {{{{SHELL}}}} is no longer supported — use $SHELL in scripts or `spawn -i` for interactive tests",
                    lineno + 1
                ));
            }
        }
    }

    let mut suite_name: Option<String> = None;
    let mut tests = Vec::new();
    let mut pending_reqs: Vec<Requirement> = Vec::new();

    let mut in_test = false;
    let mut test_interactive = false;
    let mut test_name = String::new();
    let mut test_start_line: usize = 0;
    let mut test_env: Vec<(String, String)> = Vec::new();
    let mut test_lines: Vec<(usize, String)> = Vec::new();
    let mut test_reqs: Vec<Requirement> = Vec::new();
    let mut in_script = false;
    let mut script_body: Vec<String> = Vec::new();
    let mut test_script: Option<String> = None;
    let mut in_expect = false;
    let mut expect_stdout: Option<(usize, String)> = None;
    let mut expect_stderr: Option<(usize, String)> = None;
    let mut expect_exit_code: Option<(usize, String)> = None;
    let mut expect_step: u8 = 0; // 0=awaiting stdout, 1=awaiting stderr, 2=awaiting exit_code, 3=done

    let lines_vec: Vec<&str> = text.lines().collect();
    let mut lineno = 0;

    while lineno < lines_vec.len() {
        let raw_line = lines_vec[lineno];
        let line_num = lineno + 1;

        if in_script {
            if raw_line == "  expect" {
                in_script = false;
                test_script = Some(script_body.join("\n"));
                in_expect = true;
                expect_step = 0;
                expect_stdout = None;
                expect_stderr = None;
                expect_exit_code = None;
                lineno += 1;
                continue;
            }
            if raw_line.is_empty() {
                script_body.push(String::new());
                lineno += 1;
                continue;
            }
            if let Some(stripped) = raw_line.strip_prefix("    ") {
                script_body.push(stripped.to_string());
            } else {
                return Err(format!(
                    "line {line_num}: script body must be indented by 4 spaces: {:?}",
                    raw_line
                ));
            }
            lineno += 1;
            continue;
        }

        if in_expect {
            if let Some(stripped) = raw_line.strip_prefix("    ") {
                let stripped = stripped.trim_end();
                match expect_step {
                    0 => {
                        if let Some(rest) = stripped.strip_prefix("stdout ") {
                            expect_stdout = Some((line_num, rest.to_string()));
                            expect_step = 1;
                        } else {
                            return Err(format!(
                                "line {line_num}: expected `stdout` as first assertion in expect block, got: {:?}",
                                stripped
                            ));
                        }
                    }
                    1 => {
                        if let Some(rest) = stripped.strip_prefix("stderr ") {
                            expect_stderr = Some((line_num, rest.to_string()));
                            expect_step = 2;
                        } else {
                            return Err(format!(
                                "line {line_num}: expected `stderr` as second assertion in expect block, got: {:?}",
                                stripped
                            ));
                        }
                    }
                    2 => {
                        if let Some(rest) = stripped.strip_prefix("exit_code ") {
                            expect_exit_code = Some((line_num, rest.to_string()));
                            expect_step = 3;
                        } else {
                            return Err(format!(
                                "line {line_num}: expected `exit_code` as third assertion in expect block, got: {:?}",
                                stripped
                            ));
                        }
                    }
                    _ => {
                        return Err(format!(
                            "line {line_num}: unexpected line after exit_code in expect block: {:?}",
                            stripped
                        ));
                    }
                }
                lineno += 1;
                continue;
            }
            if expect_step != 3 {
                let missing = match expect_step {
                    0 => "stdout",
                    1 => "stderr",
                    2 => "exit_code",
                    _ => unreachable!(),
                };
                return Err(format!(
                    "line {line_num}: expect block is missing `{missing}` assertion in test {:?}",
                    test_name
                ));
            }
            in_expect = false;
            // fall through to process this line normally (should be `end test`)
        }

        let line = raw_line.trim();

        if line.is_empty() || line.starts_with('#') {
            lineno += 1;
            continue;
        }

        if let Some(rest) = line.strip_prefix("testsuite ") {
            if suite_name.is_some() {
                return Err(format!("line {line_num}: duplicate testsuite directive"));
            }
            suite_name =
                Some(extract_quoted(rest.trim()).map_err(|e| format!("line {line_num}: {e}"))?);
            lineno += 1;
            continue;
        }

        if let Some(rest) = line.strip_prefix("requirement ") {
            let rest = rest.trim();
            let (id_part, remainder) = match rest.find(' ') {
                Some(pos) => (rest[..pos].trim(), rest[pos + 1..].trim()),
                None => (rest, ""),
            };
            let id = extract_quoted(id_part).map_err(|e| format!("line {line_num}: {e}"))?;
            let doc = if let Some(doc_rest) = remainder.strip_prefix("doc=") {
                extract_quoted(doc_rest.trim()).map_err(|e| format!("line {line_num}: {e}"))?
            } else {
                return Err(format!(
                    "line {line_num}: requirement {:?} is missing doc parameter",
                    id
                ));
            };
            if doc.ends_with(":.") {
                return Err(format!(
                    "line {line_num}: requirement {:?} doc must not end with \":.\"; trim the trailing colon or complete the sentence",
                    id
                ));
            }
            pending_reqs.push(Requirement { id, doc });
            lineno += 1;
            continue;
        }

        let begin_match = line
            .strip_prefix("begin interactive test ")
            .map(|rest| (true, rest))
            .or_else(|| line.strip_prefix("begin test ").map(|rest| (false, rest)));
        if let Some((interactive, rest)) = begin_match {
            if in_test {
                return Err(format!(
                    "line {line_num}: nested begin test (already in {:?})",
                    test_name
                ));
            }
            in_test = true;
            test_interactive = interactive;
            test_name = extract_quoted(rest.trim()).map_err(|e| format!("line {line_num}: {e}"))?;
            test_start_line = line_num;
            test_env.clear();
            test_lines.clear();
            test_reqs = std::mem::take(&mut pending_reqs);
            test_script = None;
            expect_stdout = None;
            expect_stderr = None;
            expect_exit_code = None;
            lineno += 1;
            continue;
        }

        let end_match = line
            .strip_prefix("end interactive test ")
            .map(|rest| (true, rest))
            .or_else(|| line.strip_prefix("end test ").map(|rest| (false, rest)));
        if let Some((interactive, rest)) = end_match {
            if !in_test {
                return Err(format!("line {line_num}: end test without begin test"));
            }
            if interactive != test_interactive {
                let expected = if test_interactive {
                    "end interactive test"
                } else {
                    "end test"
                };
                return Err(format!(
                    "line {line_num}: expected {expected} to match begin, got: {line}"
                ));
            }
            let end_name =
                extract_quoted(rest.trim()).map_err(|e| format!("line {line_num}: {e}"))?;
            if end_name != test_name {
                return Err(format!(
                    "line {line_num}: end test {:?} does not match begin test {:?}",
                    end_name, test_name
                ));
            }
            if !test_interactive && test_script.is_none() {
                return Err(format!(
                    "line {line_num}: non-interactive test {:?} has no script block",
                    test_name
                ));
            }
            if !test_interactive && expect_stdout.is_none() {
                return Err(format!(
                    "line {line_num}: non-interactive test {:?} has no expect block",
                    test_name
                ));
            }
            tests.push(TestCase {
                name: test_name.clone(),
                interactive: test_interactive,
                line_num: test_start_line,
                requirements: std::mem::take(&mut test_reqs),
                env_overrides: test_env.clone(),
                script_lines: test_lines.clone(),
                script: test_script.take(),
                expect_stdout: expect_stdout.take(),
                expect_stderr: expect_stderr.take(),
                expect_exit_code: expect_exit_code.take(),
            });
            in_test = false;
            lineno += 1;
            continue;
        }

        if in_test {
            if line == "script" {
                if test_interactive {
                    return Err(format!(
                        "line {line_num}: script block is not allowed in interactive tests"
                    ));
                }
                if test_script.is_some() {
                    return Err(format!(
                        "line {line_num}: duplicate script block in test {:?}",
                        test_name
                    ));
                }
                in_script = true;
                script_body.clear();
                lineno += 1;
                continue;
            }
            if let Some(rest) = line.strip_prefix("setenv ") {
                let rest = rest.trim();
                let key_end = find_closing_quote(rest)
                    .map_err(|e| format!("line {line_num}: setenv key: {e}"))?;
                let key = extract_quoted(&rest[..key_end + 1])
                    .map_err(|e| format!("line {line_num}: setenv key: {e}"))?;
                let val_part = rest[key_end + 1..].trim();
                let val = extract_quoted(val_part)
                    .map_err(|e| format!("line {line_num}: setenv value: {e}"))?;
                test_env.push((key, val));
            } else {
                test_lines.push((line_num, raw_line.to_string()));
            }
            lineno += 1;
            continue;
        }

        return Err(format!(
            "line {line_num}: unexpected command outside test block: {line}"
        ));
    }

    if in_script {
        return Err(format!(
            "unterminated script block in test {:?} starting at line {test_start_line}",
            test_name
        ));
    }

    if in_expect {
        return Err(format!(
            "unterminated expect block in test {:?} starting at line {test_start_line}",
            test_name
        ));
    }

    if in_test {
        return Err(format!(
            "unterminated test {:?} starting at line {test_start_line}",
            test_name
        ));
    }

    Ok(TestSuite {
        name: suite_name.unwrap_or_else(|| filename.to_string()),
        filename: filename.to_string(),
        tests,
    })
}

fn find_closing_quote(s: &str) -> Result<usize, String> {
    if !s.starts_with('"') {
        return Err(format!("expected quoted string, got: {s}"));
    }
    let mut i = 1;
    let bytes = s.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            i += 2;
            continue;
        }
        if bytes[i] == b'"' {
            return Ok(i);
        }
        i += 1;
    }
    Err(format!("unterminated quoted string: {s}"))
}

fn extract_quoted(arg: &str) -> Result<String, String> {
    let arg = arg.trim();
    if !arg.starts_with('"') || !arg.ends_with('"') || arg.len() < 2 {
        return Err(format!("expected quoted string, got: {arg}"));
    }
    let inner = &arg[1..arg.len() - 1];
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            let esc = chars
                .next()
                .ok_or_else(|| "dangling backslash in quoted string".to_string())?;
            match esc {
                '\\' => out.push('\\'),
                '"' => out.push('"'),
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                other => {
                    return Err(format!("unsupported escape sequence: \\{other}"));
                }
            }
        } else {
            out.push(ch);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closing_quote_simple() {
        assert_eq!(find_closing_quote(r#""hello""#).unwrap(), 6);
    }

    #[test]
    fn closing_quote_escaped() {
        assert_eq!(find_closing_quote(r#""say \"hi\"""#).unwrap(), 11);
    }

    #[test]
    fn closing_quote_unterminated() {
        assert!(find_closing_quote(r#""hello"#).is_err());
    }
}
