use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

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
    name.chars().next().map_or(false, |c| c.is_ascii_digit())
        || name.starts_with("utility: ")
        || name.starts_with("xbd: ")
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
                while preamble
                    .last()
                    .map_or(false, |l| l.is_empty() || l == "---")
                {
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

fn source_file_for_xbd_section(section_name: &str, md_root: &Path) -> Option<PathBuf> {
    let chapter: &str = section_name.split('.').next()?;
    let chap_num: u32 = chapter.parse().ok()?;
    let filename = format!("V1_chap{:02}.md", chap_num);
    Some(md_root.join("basedefs").join(filename))
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
            let (source_path, source_section_name) = if let Some(util_name) =
                citation.section_name.strip_prefix("utility: ")
            {
                let p = md_root.join("utilities").join(format!("{util_name}.md"));
                (p, citation.section_name.clone())
            } else if let Some(xbd_section) = citation.section_name.strip_prefix("xbd: ") {
                match source_file_for_xbd_section(xbd_section, md_root) {
                    Some(p) => (p, xbd_section.to_string()),
                    None => {
                        errors.push(format!(
                                "{filename}: line {}: cannot determine XBD source file for section {:?}",
                                citation.line_num, citation.section_name
                            ));
                        continue;
                    }
                }
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
            let source_sections =
                source_cache.entry(source_path.clone()).or_insert_with(
                    || match fs::read_to_string(&source_path_for_stem) {
                        Ok(c) => {
                            let mut secs = extract_source_sections(&c);
                            let stem = source_path_for_stem
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .map(|s| s.to_string());
                            extract_standalone_utility_page(&c, &mut secs, stem.as_deref());
                            secs
                        }
                        Err(e) => {
                            errors.push(format!(
                                "{filename}: cannot read source {}: {e}",
                                source_path.to_string_lossy()
                            ));
                            Vec::new()
                        }
                    },
                );
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
    let md_root = derive_md_root_from_matrix_dir(&matrix_dir);
    let md_files = match collect_test_files(&matrix_dir, "md") {
        Ok(v) => v,
        Err(e) => {
            eprintln!("check_integrity: {e}");
            std::process::exit(2);
        }
    };
    if md_files.is_empty() {
        eprintln!(
            "check_integrity: no .md files found under {}",
            matrix_dir.join("tests").to_string_lossy()
        );
        std::process::exit(2);
    }

    let (errors, citation_count) = check_md_citations(&md_files, &md_root);

    if !errors.is_empty() {
        for e in &errors {
            eprintln!("integrity: {e}");
        }
        eprintln!("{} integrity error(s)", errors.len());
        std::process::exit(1);
    }

    eprintln!(
        "integrity OK: {} md section citation(s) verified across {} file(s)",
        citation_count,
        md_files.len(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(sections[0].body_lines, vec!["Line one.", "", "Line two."]);
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

        let brk = sections
            .iter()
            .find(|s| s.name == "2.15 Special Built-In Utilities break")
            .unwrap();
        assert_eq!(brk.body_lines[0], "#### NAME");
        assert!(brk.body_lines.last().unwrap() == "*End of informative text.*");

        let colon = sections
            .iter()
            .find(|s| s.name == "2.15 Special Built-In Utilities colon")
            .unwrap();
        assert_eq!(colon.body_lines[0], "#### NAME");
        assert!(colon.body_lines.last().unwrap() == "*End of informative text.*");
    }

    #[test]
    fn parse_utility_desc_basic() {
        assert_eq!(
            parse_utility_desc("> break — exit from loop"),
            Some("break".to_string())
        );
        assert_eq!(
            parse_utility_desc("> colon — null utility"),
            Some("colon".to_string())
        );
        assert_eq!(parse_utility_desc("not a desc line"), None);
    }
}
