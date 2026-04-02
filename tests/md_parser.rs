use crate::epty_parser;
pub use crate::epty_parser::TestSuite;

pub fn parse_md_suite(text: &str, filename: &str) -> Result<TestSuite, String> {
    let lines: Vec<&str> = text.lines().collect();
    let total_lines = lines.len();

    // Collected epty text with blank-line padding so line numbers match the MD source.
    let mut epty_lines: Vec<String> = Vec::new();
    let mut epty_next_line = 0; // next 0-based line index expected in epty_lines

    let mut i = 0;
    while i < total_lines {
        let line = lines[i];
        if let Some(test_name) = parse_test_heading(line) {
            let heading_line = i + 1; // 1-based
            i += 1;

            let mut found_fence = false;
            let mut block_lines: Vec<(usize, &str)> = Vec::new();

            while i < total_lines {
                let cur = lines[i];

                if is_heading(cur) {
                    break;
                }

                if cur.trim() == "```" {
                    if !found_fence {
                        found_fence = true;
                        i += 1;
                        while i < total_lines {
                            let inner = lines[i];
                            if inner.trim() == "```" {
                                i += 1;
                                break;
                            }
                            if inner.contains("```") {
                                return Err(format!(
                                    "line {}: code block in test {:?} contains triple backticks inside the block",
                                    i + 1, test_name
                                ));
                            }
                            block_lines.push((i, inner));
                            i += 1;
                        }
                    } else {
                        return Err(format!(
                            "line {}: test section {:?} contains more than one code block",
                            i + 1, test_name
                        ));
                    }
                    continue;
                }

                if !found_fence || !block_lines.is_empty() {
                    // Text outside the code block — must not contain headings
                    // (already checked above via is_heading break)
                }

                i += 1;
            }

            if !found_fence || block_lines.is_empty() {
                return Err(format!(
                    "line {heading_line}: test section {:?} does not contain a code block",
                    test_name
                ));
            }

            validate_block_test_name(&test_name, &block_lines, heading_line)?;

            // Pad epty_lines so that the block content appears at the right line numbers.
            let block_start = block_lines[0].0;
            while epty_next_line < block_start {
                epty_lines.push(String::new());
                epty_next_line += 1;
            }
            for &(line_idx, content) in &block_lines {
                while epty_next_line < line_idx {
                    epty_lines.push(String::new());
                    epty_next_line += 1;
                }
                epty_lines.push(content.to_string());
                epty_next_line = line_idx + 1;
            }
        } else {
            i += 1;
        }
    }

    let epty_text = epty_lines.join("\n");
    epty_parser::parse_suite(&epty_text, filename)
}

fn parse_test_heading(line: &str) -> Option<String> {
    let rest = line.strip_prefix("##### Test: ")?;
    let name = rest.trim();
    if name.is_empty() {
        return None;
    }
    Some(name.to_string())
}

fn is_heading(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with('#') && {
        let hashes = trimmed.chars().take_while(|&c| c == '#').count();
        hashes >= 1 && hashes <= 6 && trimmed.as_bytes().get(hashes) == Some(&b' ')
    }
}

fn validate_block_test_name(
    heading_name: &str,
    block_lines: &[(usize, &str)],
    heading_line: usize,
) -> Result<(), String> {
    let mut found_begin = false;
    let mut block_test_name = String::new();

    for &(line_idx, content) in block_lines {
        let trimmed = content.trim();
        let begin_rest = trimmed
            .strip_prefix("begin interactive test ")
            .or_else(|| trimmed.strip_prefix("begin test "));
        if let Some(rest) = begin_rest {
            if found_begin {
                return Err(format!(
                    "line {}: test section {:?} contains more than one begin test",
                    line_idx + 1, heading_name
                ));
            }
            found_begin = true;
            block_test_name = extract_test_name_from_quoted(rest.trim(), line_idx + 1)?;
        }
    }

    if !found_begin {
        return Err(format!(
            "line {heading_line}: test section {:?} code block does not contain a begin test",
            heading_name
        ));
    }

    if block_test_name != heading_name {
        return Err(format!(
            "line {heading_line}: test section heading name {:?} does not match test block name {:?}",
            heading_name, block_test_name
        ));
    }

    Ok(())
}

fn extract_test_name_from_quoted(s: &str, line_num: usize) -> Result<String, String> {
    let s = s.trim();
    if !s.starts_with('"') {
        return Err(format!("line {line_num}: expected quoted test name, got: {s}"));
    }
    let mut end = 1;
    let bytes = s.as_bytes();
    while end < bytes.len() {
        if bytes[end] == b'\\' {
            end += 2;
            continue;
        }
        if bytes[end] == b'"' {
            break;
        }
        end += 1;
    }
    if end >= bytes.len() || bytes[end] != b'"' {
        return Err(format!("line {line_num}: unterminated quoted test name: {s}"));
    }
    let inner = &s[1..end];
    let mut out = String::new();
    let mut chars = inner.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(esc) = chars.next() {
                match esc {
                    '\\' => out.push('\\'),
                    '"' => out.push('"'),
                    'n' => out.push('\n'),
                    other => {
                        out.push('\\');
                        out.push(other);
                    }
                }
            }
        } else {
            out.push(ch);
        }
    }
    Ok(out)
}
