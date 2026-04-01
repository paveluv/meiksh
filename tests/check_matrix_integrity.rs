#[path = "json.rs"]
mod json;
#[path = "epty_parser.rs"]
mod epty_parser;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use epty_parser::TestSuite;

struct ReqEntry {
    id: String,
    text: String,
    file: String,
    section_path: Vec<String>,
    testable: bool,
    tests: Vec<(String, String)>,
}

fn load_requirements(path: &Path) -> Result<Vec<ReqEntry>, String> {
    let path_str = path.to_string_lossy();
    let content = fs::read_to_string(path).map_err(|e| format!("cannot read {path_str}: {e}"))?;
    let root = json::parse_json(&content).map_err(|e| format!("{path_str}: {e}"))?;
    let arr = root
        .as_array()
        .ok_or_else(|| format!("{path_str}: expected top-level JSON array"))?;
    let mut entries = Vec::with_capacity(arr.len());
    for (i, item) in arr.iter().enumerate() {
        let id = item
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("{path_str}[{i}]: missing or non-string \"id\""))?
            .to_string();
        let text = item
            .get("text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("{path_str}[{i}] ({id}): missing or non-string \"text\""))?
            .to_string();
        let file = item
            .get("file")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("{path_str}[{i}] ({id}): missing or non-string \"file\""))?
            .to_string();
        let section_path = match item.get("section_path") {
            Some(json::JsonValue::Array(arr)) => arr
                .iter()
                .enumerate()
                .map(|(j, e)| {
                    e.as_str().map(|s| s.to_string()).ok_or_else(|| {
                        format!("{path_str}[{i}].section_path[{j}]: expected string")
                    })
                })
                .collect::<Result<Vec<_>, _>>()?,
            Some(json::JsonValue::Null) | None => Vec::new(),
            _ => {
                return Err(format!(
                    "{path_str}[{i}] ({id}): \"section_path\" must be an array or null"
                ));
            }
        };
        let testable = item
            .get("testable")
            .and_then(|v| v.as_bool())
            .ok_or_else(|| format!("{path_str}[{i}] ({id}): missing or non-bool \"testable\""))?;
        let tests = match item.get("tests") {
            Some(json::JsonValue::Array(arr)) => arr
                .iter()
                .enumerate()
                .map(|(j, e)| {
                    let suite = e
                        .get("suite")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| format!("{path_str}[{i}].tests[{j}]: missing \"suite\""))?
                        .to_string();
                    let test = e
                        .get("test")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| format!("{path_str}[{i}].tests[{j}]: missing \"test\""))?
                        .to_string();
                    Ok((suite, test))
                })
                .collect::<Result<Vec<_>, String>>()?,
            Some(json::JsonValue::Null) | None => Vec::new(),
            _ => return Err(format!("{path_str}[{i}] ({id}): \"tests\" must be an array or null")),
        };
        entries.push(ReqEntry {
            id,
            text,
            file,
            section_path,
            testable,
            tests,
        });
    }
    Ok(entries)
}

struct HtmlSection {
    heading: String,
    text: String,
}

fn parse_html_sections(path: &Path) -> Result<Vec<HtmlSection>, String> {
    let path_str = path.to_string_lossy();
    let content = fs::read_to_string(path).map_err(|e| format!("cannot read {path_str}: {e}"))?;
    let mut headings: Vec<(usize, String)> = Vec::new();
    for (idx, line) in content.lines().enumerate() {
        if let Some(h) = extract_heading_text(line.trim()) {
            if !h.is_empty() {
                headings.push((idx + 1, h));
            }
        }
    }

    let lines: Vec<&str> = content.lines().collect();
    let mut sections = Vec::with_capacity(headings.len());
    for (i, (start, heading)) in headings.iter().enumerate() {
        let end = if i + 1 < headings.len() {
            headings[i + 1].0
        } else {
            lines.len() + 1
        };
        let mut text = String::new();
        for line_idx in *start..end.min(lines.len() + 1) {
            if line_idx == 0 || line_idx > lines.len() {
                continue;
            }
            let raw = strip_html_tags_inline(lines[line_idx - 1]);
            let decoded = decode_html_entities_inline(&raw);
            text.push(' ');
            text.push_str(&decoded);
        }
        sections.push(HtmlSection {
            heading: heading.clone(),
            text: collapse_whitespace(&text),
        });
    }
    Ok(sections)
}

fn extract_heading_text(line: &str) -> Option<String> {
    let lower = line.to_ascii_lowercase();
    if !(lower.starts_with("<h2")
        || lower.starts_with("<h3")
        || lower.starts_with("<h4")
        || lower.starts_with("<h5"))
    {
        return None;
    }
    let text = decode_html_entities_inline(&strip_html_tags_inline(line))
        .trim()
        .to_string();
    if text.is_empty() { None } else { Some(text) }
}

fn strip_html_tags_inline(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for ch in s.chars() {
        if ch == '<' {
            in_tag = true;
        } else if ch == '>' {
            in_tag = false;
            out.push(' ');
        } else if !in_tag {
            out.push(ch);
        }
    }
    out
}

fn decode_html_entities_inline(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.char_indices().peekable();
    while let Some((i, ch)) = chars.next() {
        if ch != '&' {
            out.push(ch);
            continue;
        }
        let mut end = None;
        let mut probe = chars.clone();
        while let Some((pi, pc)) = probe.next() {
            if pc == ';' {
                end = Some(pi);
                break;
            }
            if pc.is_whitespace() || pi - i > 16 {
                break;
            }
        }
        let Some(end_idx) = end else {
            out.push('&');
            continue;
        };
        let entity = &s[i + 1..end_idx];
        let decoded = match entity {
            "amp" => '&',
            "lt" => '<',
            "gt" => '>',
            "quot" => '"',
            "apos" => '\'',
            "nbsp" => ' ',
            _ => {
                out.push('&');
                continue;
            }
        };
        out.push(decoded);
        while let Some((pi, _)) = chars.peek() {
            if *pi <= end_idx {
                chars.next();
            } else {
                break;
            }
        }
    }
    out
}

fn collapse_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_space = true;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    out.trim().to_string()
}

fn text_found_in_section(req_text: &str, section_text: &str) -> bool {
    for needle_len in [60, 40, 30, 20] {
        if req_text.len() >= needle_len && section_text.contains(&req_text[..needle_len]) {
            return true;
        }
    }
    if section_text.contains(req_text) {
        return true;
    }
    let words: Vec<&str> = req_text.split_whitespace().collect();
    if words.len() < 3 {
        return false;
    }
    let window_count = words.len().saturating_sub(2).min(15);
    let mut hits = 0usize;
    let section_lower = section_text.to_ascii_lowercase();
    for i in 0..window_count {
        let phrase: String = words[i..i + 3].join(" ").to_ascii_lowercase();
        if section_lower.contains(&phrase) {
            hits += 1;
        }
    }
    hits * 4 >= window_count
}

fn check_requirements_integrity(
    req_entries: &[ReqEntry],
    suites: &[(String, TestSuite)],
    html_root: &Path,
) -> Vec<String> {
    let mut errors: Vec<String> = Vec::new();
    let mut req_by_id: HashMap<&str, usize> = HashMap::new();

    let mut seen_ids: HashMap<&str, usize> = HashMap::new();
    for (i, entry) in req_entries.iter().enumerate() {
        if let Some(&prev) = seen_ids.get(entry.id.as_str()) {
            errors.push(format!(
                "requirements.json: duplicate id {:?} at indices {} and {}",
                entry.id, prev, i
            ));
        } else {
            seen_ids.insert(&entry.id, i);
        }
        req_by_id.insert(&entry.id, i);
    }

    let mut seen_texts: HashMap<&str, &str> = HashMap::new();
    for entry in req_entries {
        if let Some(prev_id) = seen_texts.get(entry.text.as_str()) {
            errors.push(format!(
                "requirements.json: duplicate text shared by {:?} and {:?}",
                prev_id, entry.id
            ));
        } else {
            seen_texts.insert(&entry.text, &entry.id);
        }
    }

    let mut actual_tests: HashMap<String, Vec<(String, String)>> = HashMap::new();
    for (_file, suite) in suites {
        for test in &suite.tests {
            for req in &test.requirements {
                actual_tests
                    .entry(req.id.clone())
                    .or_default()
                    .push((suite.name.clone(), test.name.clone()));
            }
        }
    }

    for (_file, suite) in suites {
        for test in &suite.tests {
            if test.requirements.is_empty() {
                errors.push(format!(
                    "{}: test {:?} has no requirement linked to it",
                    suite.filename, test.name
                ));
            }
            if test.requirements.len() > 3 {
                errors.push(format!(
                    "{}: test {:?} has {} requirements (max 3)",
                    suite.filename,
                    test.name,
                    test.requirements.len()
                ));
            }
            for req in &test.requirements {
                let Some(&idx) = req_by_id.get(req.id.as_str()) else {
                    errors.push(format!(
                        "{}: test {:?}: requirement {:?} not found in requirements.json",
                        suite.filename, test.name, req.id
                    ));
                    continue;
                };
                let entry = &req_entries[idx];
                if req.doc != entry.text {
                    let epty_trunc: String = req.doc.chars().take(80).collect();
                    let json_trunc: String = entry.text.chars().take(80).collect();
                    errors.push(format!(
                        "{}: test {:?}: requirement {:?} doc mismatch\n  epty: {:?}{}\n  json: {:?}{}",
                        suite.filename,
                        test.name,
                        req.id,
                        epty_trunc,
                        if req.doc.len() > 80 { "..." } else { "" },
                        json_trunc,
                        if entry.text.len() > 80 { "..." } else { "" },
                    ));
                }
                if !entry.testable {
                    errors.push(format!(
                        "{}: test {:?}: requirement {:?} is marked untestable in requirements.json",
                        suite.filename, test.name, req.id
                    ));
                }
            }
        }
    }

    for entry in req_entries {
        let actual = actual_tests.get(&entry.id);
        let actual_set: HashSet<(&str, &str)> = actual
            .map(|v| v.iter().map(|(s, t)| (s.as_str(), t.as_str())).collect())
            .unwrap_or_default();
        let json_set: HashSet<(&str, &str)> = entry
            .tests
            .iter()
            .map(|(s, t)| (s.as_str(), t.as_str()))
            .collect();

        for &(suite, test) in &json_set {
            if !actual_set.contains(&(suite, test)) {
                errors.push(format!(
                    "requirements.json: {:?} lists test ({:?}, {:?}) but no such link exists in .epty files",
                    entry.id, suite, test
                ));
            }
        }
        for &(suite, test) in &actual_set {
            if !json_set.contains(&(suite, test)) {
                errors.push(format!(
                    "requirements.json: {:?} is missing test ({:?}, {:?}) from its \"tests\" list",
                    entry.id, suite, test
                ));
            }
        }
        if entry.testable && actual_set.is_empty() {
            errors.push(format!(
                "requirements.json: testable requirement {:?} has no tests linked",
                entry.id
            ));
        }
    }

    let mut file_cache: HashMap<String, Option<Vec<HtmlSection>>> = HashMap::new();
    for entry in req_entries {
        if entry.section_path.is_empty() || entry.file.is_empty() {
            continue;
        }
        let sections = file_cache.entry(entry.file.clone()).or_insert_with(|| {
            let html_path = html_root.join(&entry.file);
            parse_html_sections(&html_path).ok()
        });
        let Some(sections) = sections else {
            continue;
        };
        let leaf = &entry.section_path[entry.section_path.len() - 1];
        let matching_sections: Vec<&HtmlSection> =
            sections.iter().filter(|s| s.heading == *leaf).collect();
        if matching_sections.is_empty() {
            errors.push(format!(
                "requirements.json: {:?} section_path leaf {:?} not found as heading in {}",
                entry.id, leaf, entry.file
            ));
            continue;
        }
        let normalized_req = collapse_whitespace(&entry.text);
        let found = matching_sections
            .iter()
            .any(|s| text_found_in_section(&normalized_req, &s.text));
        if !found {
            errors.push(format!(
                "requirements.json: {:?} text not found under section {:?} in {}",
                entry.id, leaf, entry.file
            ));
        }
    }
    errors
}

fn collect_epty_files(matrix_dir: &Path) -> Result<Vec<PathBuf>, String> {
    let tests_dir = matrix_dir.join("tests");
    let rd = fs::read_dir(&tests_dir)
        .map_err(|e| format!("cannot read {}: {e}", tests_dir.to_string_lossy()))?;
    let mut files = Vec::new();
    for ent in rd {
        let ent = ent.map_err(|e| format!("cannot read entry in {}: {e}", tests_dir.to_string_lossy()))?;
        let p = ent.path();
        if p.extension().and_then(|s| s.to_str()) == Some("epty") {
            files.push(p);
        }
    }
    files.sort();
    Ok(files)
}

fn derive_html_root_from_matrix_dir(matrix_dir: &Path) -> PathBuf {
    // matrix_dir is expected to be <repo>/tests/matrix
    if let Some(repo_root) = matrix_dir.parent().and_then(|p| p.parent()) {
        repo_root.join("docs/posix/susv5-html")
    } else {
        PathBuf::from("docs/posix/susv5-html")
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("usage: check_integrity <matrix_dir>");
        std::process::exit(2);
    }

    let matrix_dir = PathBuf::from(&args[1]);
    let req_path = matrix_dir.join("requirements.json");
    let html_root = derive_html_root_from_matrix_dir(&matrix_dir);
    let epty_files = match collect_epty_files(&matrix_dir) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("check_integrity: {e}");
            std::process::exit(2);
        }
    };
    if epty_files.is_empty() {
        eprintln!(
            "check_integrity: no .epty files found under {}",
            matrix_dir.join("tests").to_string_lossy()
        );
        std::process::exit(2);
    }

    let mut suites: Vec<(String, TestSuite)> = Vec::new();
    let mut parse_errors = 0usize;
    for file in &epty_files {
        let text = match fs::read_to_string(file) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("check_integrity: cannot read {}: {e}", file.to_string_lossy());
                std::process::exit(2);
            }
        };
        let filename = file.file_name().and_then(|s| s.to_str()).unwrap_or("unknown.epty");
        match epty_parser::parse_suite(&text, filename) {
            Ok(s) => suites.push((file.to_string_lossy().to_string(), s)),
            Err(e) => {
                eprintln!("check_integrity: parse error in {}: {e}", file.to_string_lossy());
                parse_errors += 1;
            }
        }
    }
    if parse_errors > 0 {
        eprintln!("{parse_errors} file(s) had parse errors");
        std::process::exit(1);
    }

    let req_entries = match load_requirements(&req_path) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("check_integrity: {e}");
            std::process::exit(2);
        }
    };
    let errs = check_requirements_integrity(&req_entries, &suites, &html_root);
    if !errs.is_empty() {
        for e in &errs {
            eprintln!("integrity: {e}");
        }
        eprintln!("{} integrity error(s)", errs.len());
        std::process::exit(1);
    }

    eprintln!(
        "integrity OK for {} file(s) ({} tests)",
        epty_files.len(),
        suites.iter().map(|(_, s)| s.tests.len()).sum::<usize>()
    );
}
