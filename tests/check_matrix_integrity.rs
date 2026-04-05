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

fn collect_test_files(matrix_dir: &Path, ext: &str) -> Result<Vec<PathBuf>, String> {
    let tests_dir = matrix_dir.join("tests");
    let rd = fs::read_dir(&tests_dir)
        .map_err(|e| format!("cannot read {}: {e}", tests_dir.to_string_lossy()))?;
    let mut files = Vec::new();
    for ent in rd {
        let ent =
            ent.map_err(|e| format!("cannot read entry in {}: {e}", tests_dir.to_string_lossy()))?;
        let p = ent.path();
        if p.extension().and_then(|s| s.to_str()) == Some(ext) {
            files.push(p);
        }
    }
    files.sort();
    Ok(files)
}

struct MdCitation {
    section_name: String,
    body_lines: Vec<String>,
    line_num: usize,
}

fn is_citation_heading(name: &str) -> bool {
    name.chars().next().map_or(false, |c| c.is_ascii_digit()) || name.starts_with("utility: ")
}

fn extract_md_citations(content: &str) -> Vec<MdCitation> {
    let lines: Vec<&str> = content.lines().collect();
    let mut citations = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if !line.starts_with("## ") || line.starts_with("## Table of contents") {
            i += 1;
            continue;
        }
        let section_name = line[3..].trim().to_string();
        if !is_citation_heading(&section_name) {
            i += 1;
            continue;
        }
        let heading_line = i + 1;
        i += 1;
        let mut body = Vec::new();
        while i < lines.len() {
            let l = lines[i];
            if l.starts_with("## ") || l.starts_with("### Tests") {
                break;
            }
            body.push(l.to_string());
            i += 1;
        }
        while body.last().map_or(false, |l| l.is_empty()) {
            body.pop();
        }
        while body.first().map_or(false, |l| l.is_empty()) {
            body.remove(0);
        }
        if !body.is_empty() {
            citations.push(MdCitation {
                section_name,
                body_lines: body,
                line_num: heading_line,
            });
        }
    }
    citations
}

struct SourceSection {
    name: String,
    body_lines: Vec<String>,
}

fn extract_source_sections(content: &str) -> Vec<SourceSection> {
    let lines: Vec<&str> = content.lines().collect();
    let mut headings: Vec<(usize, String)> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if !(line.starts_with("## ")
            || line.starts_with("### ")
            || line.starts_with("#### ")
            || line.starts_with("##### "))
        {
            continue;
        }
        let hashes = line.bytes().take_while(|&b| b == b'#').count();
        let name = line[hashes..].trim().to_string();
        if name.chars().next().map_or(false, |c| c.is_ascii_digit()) {
            headings.push((i, name));
        }
    }

    let mut sections = Vec::new();
    for (idx, (start, name)) in headings.iter().enumerate() {
        let end = if idx + 1 < headings.len() {
            headings[idx + 1].0
        } else {
            lines.len()
        };
        let mut body: Vec<String> = lines[start + 1..end]
            .iter()
            .map(|l| l.to_string())
            .collect();
        while body.last().map_or(false, |l| l.is_empty()) {
            body.pop();
        }
        while body.first().map_or(false, |l| l.is_empty()) {
            body.remove(0);
        }
        sections.push(SourceSection {
            name: name.clone(),
            body_lines: body,
        });
    }

    let utility_parent_sections = extract_utility_pages(&lines, &mut sections);

    for parent_name in &utility_parent_sections {
        if let Some(sec) = sections.iter_mut().find(|s| s.name == *parent_name) {
            if let Some(cut) = sec.body_lines.iter().position(|l| l == "#### NAME") {
                let mut preamble = sec.body_lines[..cut].to_vec();
                while preamble.last().map_or(false, |l| l.is_empty() || l == "---") {
                    preamble.pop();
                }
                sec.body_lines = preamble;
            }
        }
    }

    sections
}

/// Extract individual utility pages from within composite sections (e.g. 2.15).
///
/// Each utility page in the source starts with `#### NAME` followed by
/// `> <name> — <description>` and ends with `*End of informative text.*`.
/// We emit a SourceSection named `<parent_section> <utility_name>`, e.g.
/// `2.15 Special Built-In Utilities break`.
///
/// Returns the set of parent section names that had utility pages extracted.
fn extract_utility_pages(lines: &[&str], sections: &mut Vec<SourceSection>) -> Vec<String> {
    let mut parent_sections = Vec::new();
    let name_indices: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter_map(|(i, l)| if *l == "#### NAME" { Some(i) } else { None })
        .collect();

    for &name_idx in &name_indices {
        let desc_idx = name_idx + 2;
        if desc_idx >= lines.len() {
            continue;
        }
        let desc_line = lines[desc_idx];
        let util_name = match parse_utility_desc(desc_line) {
            Some(n) => n,
            None => continue,
        };

        let parent = match find_parent_numbered_section(lines, name_idx) {
            Some(s) => s,
            None => continue,
        };

        let end_marker = "*End of informative text.*";
        let end_idx = (name_idx..lines.len())
            .find(|&i| lines[i] == end_marker)
            .map(|i| i + 1)
            .unwrap_or(lines.len());

        let mut body: Vec<String> = lines[name_idx..end_idx]
            .iter()
            .map(|l| l.to_string())
            .collect();
        while body.last().map_or(false, |l| l.is_empty()) {
            body.pop();
        }

        let section_name = format!("{parent} {util_name}");
        if !parent_sections.contains(&parent) {
            parent_sections.push(parent);
        }
        sections.push(SourceSection {
            name: section_name,
            body_lines: body,
        });
    }
    parent_sections
}

/// Extract a utility page from a standalone utility file (e.g. `alias.md`).
///
/// The file starts with `# <name>`, then `#### NAME`, `> <name> — <desc>`, etc.
/// We extract from `#### NAME` to `*End of informative text.*` and emit a
/// SourceSection named `utility: <name>`.
///
/// If `filename_stem` differs from the parsed utility name (e.g. `[.md` whose
/// NAME line says `test`), an additional section with the filename-based name
/// is emitted so that citations using either name resolve correctly.
fn extract_standalone_utility_page(
    content: &str,
    sections: &mut Vec<SourceSection>,
    filename_stem: Option<&str>,
) {
    let lines: Vec<&str> = content.lines().collect();
    let name_idx = match lines.iter().position(|l| *l == "#### NAME") {
        Some(i) => i,
        None => return,
    };
    let desc_idx = name_idx + 2;
    if desc_idx >= lines.len() {
        return;
    }
    let util_name = match parse_utility_desc(lines[desc_idx]) {
        Some(n) => n,
        None => return,
    };
    let end_marker = "*End of informative text.*";
    let end_idx = (name_idx..lines.len())
        .find(|&i| lines[i] == end_marker)
        .map(|i| i + 1)
        .unwrap_or(lines.len());

    let mut body: Vec<String> = lines[name_idx..end_idx]
        .iter()
        .map(|l| l.to_string())
        .collect();
    while body.last().map_or(false, |l| l.is_empty()) {
        body.pop();
    }

    sections.push(SourceSection {
        name: format!("utility: {util_name}"),
        body_lines: body.clone(),
    });

    if let Some(stem) = filename_stem {
        if stem != util_name {
            sections.push(SourceSection {
                name: format!("utility: {stem}"),
                body_lines: body,
            });
        }
    }
}

/// Parse `> name — description` into the utility name.
fn parse_utility_desc(line: &str) -> Option<String> {
    let stripped = line.strip_prefix("> ")?;
    let dash_pos = stripped.find(" — ")?;
    let raw_name = stripped[..dash_pos].trim();
    let name = raw_name
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .and_then(|s| s.strip_prefix('*'))
        .and_then(|s| s.strip_suffix('*'))
        .unwrap_or(raw_name);
    Some(name.to_string())
}

/// Walk backwards from `pos` to find the nearest numbered section heading.
fn find_parent_numbered_section(lines: &[&str], pos: usize) -> Option<String> {
    for i in (0..pos).rev() {
        let line = lines[i];
        let prefix = if line.starts_with("### ") {
            Some(4)
        } else if line.starts_with("## ") {
            Some(3)
        } else {
            None
        };
        if let Some(skip) = prefix {
            let name = line[skip..].trim();
            if name.chars().next().map_or(false, |c| c.is_ascii_digit()) {
                return Some(name.to_string());
            }
        }
    }
    None
}

fn source_file_for_section(section_name: &str, md_root: &Path) -> Option<PathBuf> {
    let chapter: &str = section_name.split('.').next()?;
    let chap_num: u32 = chapter.parse().ok()?;
    let filename = format!("V3_chap{:02}.md", chap_num);
    Some(md_root.join("utilities").join(filename))
}

fn check_md_citations(md_files: &[PathBuf], md_root: &Path) -> (Vec<String>, usize) {
    let mut errors = Vec::new();
    let mut source_cache: HashMap<PathBuf, Vec<SourceSection>> = HashMap::new();
    let mut total_citations = 0usize;

    for md_file in md_files {
        let filename = md_file
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown.md");
        let content = match fs::read_to_string(md_file) {
            Ok(c) => c,
            Err(e) => {
                errors.push(format!("{filename}: cannot read: {e}"));
                continue;
            }
        };
        let citations = extract_md_citations(&content);
        if citations.is_empty() {
            errors.push(format!("{filename}: no section citations found"));
            continue;
        }

        for citation in &citations {
            let (source_path, source_section_name) =
                if let Some(util_name) = citation.section_name.strip_prefix("utility: ") {
                    let p = md_root.join("utilities").join(format!("{util_name}.md"));
                    (p, citation.section_name.clone())
                } else {
                    match source_file_for_section(&citation.section_name, md_root) {
                        Some(p) => (p, citation.section_name.clone()),
                        None => {
                            errors.push(format!(
                                "{filename}: line {}: cannot determine source file for section {:?}",
                                citation.line_num, citation.section_name
                            ));
                            continue;
                        }
                    }
                };
            let source_path_for_stem = source_path.clone();
            let source_sections = source_cache.entry(source_path.clone()).or_insert_with(|| {
                match fs::read_to_string(&source_path_for_stem) {
                    Ok(c) => {
                        let mut secs = extract_source_sections(&c);
                        let stem = source_path_for_stem
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .map(|s| s.to_string());
                        extract_standalone_utility_page(
                            &c,
                            &mut secs,
                            stem.as_deref(),
                        );
                        secs
                    }
                    Err(e) => {
                        errors.push(format!(
                            "{filename}: cannot read source {}: {e}",
                            source_path.to_string_lossy()
                        ));
                        Vec::new()
                    }
                }
            });
            let source = match source_sections
                .iter()
                .find(|s| s.name == source_section_name)
            {
                Some(s) => s,
                None => {
                    errors.push(format!(
                        "{filename}: line {}: section {:?} not found in source {}",
                        citation.line_num,
                        citation.section_name,
                        source_path.to_string_lossy()
                    ));
                    continue;
                }
            };

            let cite = &citation.body_lines;
            let src = &source.body_lines;
            total_citations += 1;
            if cite == src {
                continue;
            }

            let cite_len = cite.len();
            let src_len = src.len();
            let mut first_diff = None;
            for j in 0..cite_len.min(src_len) {
                if cite[j] != src[j] {
                    first_diff = Some(j);
                    break;
                }
            }
            let diff_line = first_diff.unwrap_or(cite_len.min(src_len));
            let file_line = citation.line_num + 1 + diff_line;
            if let Some(j) = first_diff {
                let cite_trunc: String = cite[j].chars().take(120).collect();
                let src_trunc: String = src[j].chars().take(120).collect();
                errors.push(format!(
                    "{filename}: line {file_line}: section {:?} citation differs from source\n\
                     \x20 test suite: {:?}{}\n\
                     \x20 source:     {:?}{}",
                    citation.section_name,
                    cite_trunc,
                    if cite[j].len() > 120 { "..." } else { "" },
                    src_trunc,
                    if src[j].len() > 120 { "..." } else { "" },
                ));
            } else {
                let direction = if cite_len < src_len {
                    "shorter"
                } else {
                    "longer"
                };
                errors.push(format!(
                    "{filename}: line {}: section {:?} citation is {direction} than source ({cite_len} vs {src_len} lines)",
                    citation.line_num,
                    citation.section_name,
                ));
            }
        }
    }
    (errors, total_citations)
}

fn derive_html_root_from_matrix_dir(matrix_dir: &Path) -> PathBuf {
    if let Some(repo_root) = matrix_dir.parent().and_then(|p| p.parent()) {
        repo_root.join("docs/posix/susv5-html")
    } else {
        PathBuf::from("docs/posix/susv5-html")
    }
}

fn derive_md_root_from_matrix_dir(matrix_dir: &Path) -> PathBuf {
    if let Some(repo_root) = matrix_dir.parent().and_then(|p| p.parent()) {
        repo_root.join("docs/posix/md")
    } else {
        PathBuf::from("docs/posix/md")
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
    let md_root = derive_md_root_from_matrix_dir(&matrix_dir);
    let epty_files = match collect_test_files(&matrix_dir, "epty") {
        Ok(v) => v,
        Err(e) => {
            eprintln!("check_integrity: {e}");
            std::process::exit(2);
        }
    };
    let md_files = match collect_test_files(&matrix_dir, "md") {
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
        let filename = file
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown.epty");
        match epty_parser::parse_suite(&text, filename) {
            Ok(s) => suites.push((file.to_string_lossy().to_string(), s)),
            Err(e) => {
                eprintln!(
                    "check_integrity: parse error in {}: {e}",
                    file.to_string_lossy()
                );
                parse_errors += 1;
            }
        }
    }
    if parse_errors > 0 {
        eprintln!("{parse_errors} file(s) had parse errors");
        std::process::exit(1);
    }

    let mut all_errors = Vec::new();

    let req_entries = match load_requirements(&req_path) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("check_integrity: {e}");
            std::process::exit(2);
        }
    };
    all_errors.extend(check_requirements_integrity(
        &req_entries, &suites, &html_root,
    ));

    let mut citation_count = 0usize;
    if !md_files.is_empty() {
        let (errs, count) = check_md_citations(&md_files, &md_root);
        all_errors.extend(errs);
        citation_count = count;
    }

    if !all_errors.is_empty() {
        for e in &all_errors {
            eprintln!("integrity: {e}");
        }
        eprintln!("{} integrity error(s)", all_errors.len());
        std::process::exit(1);
    }

    eprintln!(
        "integrity OK for {} file(s) ({} tests), {} md section citation(s) verified across {} file(s)",
        epty_files.len(),
        suites.iter().map(|(_, s)| s.tests.len()).sum::<usize>(),
        citation_count,
        md_files.len(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::epty_parser::{Requirement, TestCase};

    fn make_req_entry(id: &str, text: &str, testable: bool, tests: &[(&str, &str)]) -> ReqEntry {
        ReqEntry {
            id: id.to_string(),
            text: text.to_string(),
            file: String::new(),
            section_path: Vec::new(),
            testable,
            tests: tests
                .iter()
                .map(|(suite, test)| (suite.to_string(), test.to_string()))
                .collect(),
        }
    }

    fn make_suite(name: &str, filename: &str, tests: Vec<(&str, Vec<(&str, &str)>)>) -> TestSuite {
        TestSuite {
            name: name.to_string(),
            filename: filename.to_string(),
            tests: tests
                .into_iter()
                .map(|(test_name, reqs)| TestCase {
                    name: test_name.to_string(),
                    interactive: false,
                    line_num: 1,
                    requirements: reqs
                        .into_iter()
                        .map(|(id, doc)| Requirement {
                            id: id.to_string(),
                            doc: doc.to_string(),
                        })
                        .collect(),
                    env_overrides: vec![],
                    script_lines: vec![],
                    script: Some("true".to_string()),
                    expect_stdout: None,
                    expect_stderr: None,
                    expect_exit_code: None,
                })
                .collect(),
        }
    }

    #[test]
    fn integrity_ok() {
        let reqs = vec![make_req_entry(
            "REQ-1",
            "Some text.",
            true,
            &[("Suite A", "test one")],
        )];
        let suite = make_suite(
            "Suite A",
            "a.epty",
            vec![("test one", vec![("REQ-1", "Some text.")])],
        );
        let errs = check_requirements_integrity(&reqs, &[("a.epty".into(), suite)], Path::new("/"));
        assert!(errs.is_empty(), "expected no errors, got: {errs:?}");
    }

    #[test]
    fn integrity_doc_mismatch() {
        let reqs = vec![make_req_entry("REQ-1", "Correct text.", true, &[("S", "t")])];
        let suite = make_suite("S", "s.epty", vec![("t", vec![("REQ-1", "Wrong text.")])]);
        let errs = check_requirements_integrity(&reqs, &[("s.epty".into(), suite)], Path::new("/"));
        assert!(errs.iter().any(|e| e.contains("doc mismatch")), "got: {errs:?}");
    }

    #[test]
    fn integrity_untestable() {
        let reqs = vec![make_req_entry("REQ-1", "Text.", false, &[])];
        let suite = make_suite("S", "s.epty", vec![("t", vec![("REQ-1", "Text.")])]);
        let errs = check_requirements_integrity(&reqs, &[("s.epty".into(), suite)], Path::new("/"));
        assert!(errs.iter().any(|e| e.contains("untestable")), "got: {errs:?}");
    }

    #[test]
    fn integrity_req_not_in_json() {
        let reqs = vec![];
        let suite = make_suite("S", "s.epty", vec![("t", vec![("REQ-X", "Text.")])]);
        let errs = check_requirements_integrity(&reqs, &[("s.epty".into(), suite)], Path::new("/"));
        assert!(
            errs.iter()
                .any(|e| e.contains("not found in requirements.json")),
            "got: {errs:?}"
        );
    }

    #[test]
    fn integrity_duplicate_ids_and_texts() {
        let reqs = vec![
            make_req_entry("REQ-1", "Same text.", true, &[]),
            make_req_entry("REQ-1", "Same text.", true, &[]),
        ];
        let errs = check_requirements_integrity(&reqs, &[], Path::new("/"));
        assert!(errs.iter().any(|e| e.contains("duplicate id")), "got: {errs:?}");
        assert!(errs.iter().any(|e| e.contains("duplicate text")), "got: {errs:?}");
    }

    #[test]
    fn integrity_testable_no_tests() {
        let reqs = vec![make_req_entry("REQ-1", "Text.", true, &[])];
        let errs = check_requirements_integrity(&reqs, &[], Path::new("/"));
        assert!(
            errs.iter().any(|e| e.contains("has no tests linked")),
            "got: {errs:?}"
        );
    }

    #[test]
    fn integrity_json_extra_and_missing_test_pairs() {
        let reqs = vec![make_req_entry("REQ-1", "Text.", true, &[("S", "t"), ("S", "ghost")])];
        let suite = make_suite("S", "s.epty", vec![("t", vec![("REQ-1", "Text.")])]);
        let errs = check_requirements_integrity(&reqs, &[("s.epty".into(), suite)], Path::new("/"));
        assert!(
            errs.iter()
                .any(|e| e.contains("ghost") && e.contains("no such link")),
            "got: {errs:?}"
        );
        assert!(
            !errs.iter().any(|e| e.contains("is missing test")),
            "unexpected missing-link error: {errs:?}"
        );
    }

    #[test]
    fn extract_md_citations_basic() {
        let md = "\
# Test Suite for 2.1 Intro

## Table of contents

- [2.1 Intro](#21-intro)

## 2.1 Intro

First paragraph.

Second paragraph.

### Tests

#### Test: some test
";
        let citations = extract_md_citations(md);
        assert_eq!(citations.len(), 1);
        assert_eq!(citations[0].section_name, "2.1 Intro");
        assert_eq!(
            citations[0].body_lines,
            vec!["First paragraph.", "", "Second paragraph."]
        );
    }

    #[test]
    fn extract_md_citations_multiple_sections() {
        let md = "\
## 2.1 Top

Top body.

### Tests

## 2.1.1 Sub

Sub body line 1.
Sub body line 2.

### Tests
";
        let citations = extract_md_citations(md);
        assert_eq!(citations.len(), 2);
        assert_eq!(citations[0].section_name, "2.1 Top");
        assert_eq!(citations[0].body_lines, vec!["Top body."]);
        assert_eq!(citations[1].section_name, "2.1.1 Sub");
        assert_eq!(
            citations[1].body_lines,
            vec!["Sub body line 1.", "Sub body line 2."]
        );
    }

    #[test]
    fn extract_md_citations_skips_non_section() {
        let md = "\
## Table of contents

- link

## Not a number

Some text.
";
        let citations = extract_md_citations(md);
        assert!(citations.is_empty());
    }

    #[test]
    fn extract_source_sections_basic() {
        let src = "\
### 2.1 Top

Top body.

#### 2.1.1 Sub

Sub body.

### 2.2 Next

Next body.
";
        let sections = extract_source_sections(src);
        assert_eq!(sections.len(), 3);
        assert_eq!(sections[0].name, "2.1 Top");
        assert_eq!(sections[0].body_lines, vec!["Top body."]);
        assert_eq!(sections[1].name, "2.1.1 Sub");
        assert_eq!(sections[1].body_lines, vec!["Sub body."]);
        assert_eq!(sections[2].name, "2.2 Next");
        assert_eq!(sections[2].body_lines, vec!["Next body."]);
    }

    #[test]
    fn extract_source_sections_stops_at_next_heading() {
        let src = "\
### 2.1 Sec

Line one.

Line two.

#### 2.1.1 Sub

Sub text.
";
        let sections = extract_source_sections(src);
        assert_eq!(sections[0].name, "2.1 Sec");
        assert_eq!(
            sections[0].body_lines,
            vec!["Line one.", "", "Line two."]
        );
    }

    #[test]
    fn source_file_for_section_chapter2() {
        let root = Path::new("/docs/posix/md");
        let p = source_file_for_section("2.6.2 Parameter Expansion", root).unwrap();
        assert_eq!(p, PathBuf::from("/docs/posix/md/utilities/V3_chap02.md"));
    }

    #[test]
    fn source_file_for_section_chapter1() {
        let root = Path::new("/docs/posix/md");
        let p = source_file_for_section("1.3 Something", root).unwrap();
        assert_eq!(p, PathBuf::from("/docs/posix/md/utilities/V3_chap01.md"));
    }

    #[test]
    fn extract_utility_pages_from_section() {
        let src = "\
### 2.15 Special Built-In Utilities

Preamble text.

---

---

#### NAME

> break — exit from for, while, or until loop

#### DESCRIPTION

> The break utility shall exit from the nth enclosing loop.

---

*The following sections are informative.*

#### RATIONALE

> None.

*End of informative text.*

---

#### NAME

> colon — null utility

#### DESCRIPTION

> This utility shall do nothing except return a 0 exit status.

---

*The following sections are informative.*

#### RATIONALE

> None.

*End of informative text.*
";
        let sections = extract_source_sections(src);
        assert_eq!(sections[0].name, "2.15 Special Built-In Utilities");
        assert_eq!(sections[0].body_lines, vec!["Preamble text."]);

        let brk = sections.iter().find(|s| s.name == "2.15 Special Built-In Utilities break").unwrap();
        assert_eq!(brk.body_lines[0], "#### NAME");
        assert!(brk.body_lines.last().unwrap() == "*End of informative text.*");

        let colon = sections.iter().find(|s| s.name == "2.15 Special Built-In Utilities colon").unwrap();
        assert_eq!(colon.body_lines[0], "#### NAME");
        assert!(colon.body_lines.last().unwrap() == "*End of informative text.*");
    }

    #[test]
    fn parse_utility_desc_basic() {
        assert_eq!(parse_utility_desc("> break — exit from loop"), Some("break".to_string()));
        assert_eq!(parse_utility_desc("> colon — null utility"), Some("colon".to_string()));
        assert_eq!(parse_utility_desc("not a desc line"), None);
    }
}
