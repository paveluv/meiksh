use std::env;
use std::error::Error;
use std::fmt::Write as _;
use std::fs;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

#[derive(Clone, Debug)]
enum Node {
    Element(Element),
    Text(String),
}

#[derive(Clone, Debug)]
struct Element {
    name: String,
    attrs: Vec<(String, String)>,
    children: Vec<Node>,
}

#[derive(Debug)]
struct OpenElement {
    name: String,
    attrs: Vec<(String, String)>,
    children: Vec<Node>,
}

#[derive(Debug)]
struct ParsedTag {
    name: String,
    attrs: Vec<(String, String)>,
    self_closing: bool,
}

static CURRENT_INPUT_PATH: OnceLock<PathBuf> = OnceLock::new();
static HEADING_SLUG_CACHE: OnceLock<Mutex<HashMap<PathBuf, HashMap<String, String>>>> = OnceLock::new();

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let input = args.next().ok_or("usage: html_to_md <input.html> <output.md>")?;
    let output = args.next().ok_or("usage: html_to_md <input.html> <output.md>")?;
    if args.next().is_some() {
        return Err("usage: html_to_md <input.html> <output.md>".into());
    }

    let input_path = PathBuf::from(&input);
    let _ = CURRENT_INPUT_PATH.set(input_path);

    let html = fs::read_to_string(&input)?;
    let document = parse_html(&html);
    let markdown = render_document(&document);

    if let Some(parent) = Path::new(&output).parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output, markdown)?;
    Ok(())
}

fn parse_html(input: &str) -> Node {
    let mut stack = vec![OpenElement {
        name: "root".to_string(),
        attrs: Vec::new(),
        children: Vec::new(),
    }];
    let lower = input.to_ascii_lowercase();
    let bytes = input.as_bytes();
    let mut i = 0;

    while i < input.len() {
        if bytes[i] != b'<' {
            let next = input[i..].find('<').map(|offset| i + offset).unwrap_or(input.len());
            push_text(&mut stack, &input[i..next]);
            i = next;
            continue;
        }

        if lower[i..].starts_with("<!--") {
            if let Some(end) = lower[i + 4..].find("-->") {
                i += 4 + end + 3;
            } else {
                break;
            }
            continue;
        }

        if lower[i..].starts_with("</") {
            if let Some(end) = find_tag_end(input, i + 2) {
                let raw = input[i + 2..end].trim();
                let name = parse_tag_name(raw);
                if !name.is_empty() {
                    close_to(&mut stack, &name);
                }
                i = end + 1;
            } else {
                break;
            }
            continue;
        }

        if lower[i..].starts_with("<!") || lower[i..].starts_with("<?") {
            if let Some(end) = find_tag_end(input, i + 1) {
                i = end + 1;
            } else {
                break;
            }
            continue;
        }

        if let Some(end) = find_tag_end(input, i + 1) {
            let raw = &input[i + 1..end];
            if let Some(tag) = parse_start_tag(raw) {
                if tag.name == "script" || tag.name == "style" {
                    if let Some(close_end) = find_case_insensitive_closing_tag(input, &lower, end + 1, &tag.name) {
                        i = close_end;
                    } else {
                        i = end + 1;
                    }
                    continue;
                }

                implicitly_close_for_start(&mut stack, &tag.name);
                if tag.self_closing || is_void_tag(&tag.name) {
                    push_node(
                        &mut stack,
                        Node::Element(Element {
                            name: tag.name,
                            attrs: tag.attrs,
                            children: Vec::new(),
                        }),
                    );
                } else {
                    stack.push(OpenElement {
                        name: tag.name,
                        attrs: tag.attrs,
                        children: Vec::new(),
                    });
                }
            }
            i = end + 1;
        } else {
            push_text(&mut stack, &input[i..]);
            break;
        }
    }

    while stack.len() > 1 {
        pop_open_element(&mut stack);
    }

    let root = stack.pop().unwrap();
    Node::Element(Element {
        name: root.name,
        attrs: root.attrs,
        children: root.children,
    })
}

fn render_document(document: &Node) -> String {
    let title = find_first_element_text(document, "title").map(|title| normalize_inline(&title));
    let root = match document {
        Node::Element(element) => element,
        Node::Text(_) => return String::new(),
    };
    let body = find_first_element(root, "body").unwrap_or(root);
    let children = trim_to_first_heading(&body.children);

    let mut out = String::new();
    if let Some(title) = title {
        if !title.is_empty() {
            append_block(&mut out, &format!("# {title}"));
        }
    }
    render_blocks(children, &mut out);
    tidy_markdown(out)
}

fn render_blocks(nodes: &[Node], out: &mut String) {
    let mut index = 0;
    while index < nodes.len() {
        if let Some((next_index, inline_text)) = render_inline_run(nodes, index) {
            if !inline_text.is_empty() {
                append_block(out, &inline_text);
            }
            index = next_index;
            continue;
        }

        match &nodes[index] {
            Node::Text(_) => {}
            Node::Element(element) => {
                if should_skip_element(element) {
                    index += 1;
                    continue;
                }

                match element.name.as_str() {
                    "html" | "head" | "body" | "div" | "span" | "font" | "basefont" | "center" | "section" => {
                        render_blocks(&element.children, out);
                    }
                    "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                        let level = heading_level(&element.name);
                        let text = normalize_inline(&render_inline_children(&element.children));
                        if !text.is_empty() {
                            append_block(out, &format!("{} {text}", "#".repeat(level)));
                        }
                    }
                    "p" => {
                        if let Some(code_block) = try_render_multiline_code(element) {
                            append_block(out, &code_block);
                        } else {
                            let text = normalize_inline(&render_inline_children(&element.children));
                            if !text.is_empty() {
                                append_block(out, &text);
                            }
                        }
                    }
                    "pre" => {
                        let text = render_preformatted(element);
                        let text = text.trim_matches('\n');
                        if !text.trim().is_empty() {
                            append_block(out, &format!("```\n{text}\n```"));
                        }
                    }
                    "ul" => {
                        let text = render_list(element, false, 0);
                        if !text.trim().is_empty() {
                            append_block(out, text.trim_end());
                        }
                    }
                    "ol" => {
                        let text = render_list(element, true, 0);
                        if !text.trim().is_empty() {
                            append_block(out, text.trim_end());
                        }
                    }
                    "dl" => {
                        let text = render_definition_list(element);
                        if !text.trim().is_empty() {
                            append_block(out, text.trim_end());
                        }
                    }
                    "table" => {
                        let text = render_table(element);
                        if !text.trim().is_empty() {
                            append_block(out, text.trim_end());
                        }
                    }
                    "blockquote" => {
                        if let Some(code_block) = try_render_multiline_code(element) {
                            let quoted = prefix_lines(&code_block, "> ");
                            append_block(out, &quoted);
                        } else {
                            let mut nested = String::new();
                            render_blocks(&element.children, &mut nested);
                            let quoted = prefix_lines(nested.trim(), "> ");
                            if !quoted.trim().is_empty() {
                                append_block(out, &quoted);
                            }
                        }
                    }
                    "hr" => append_block(out, "---"),
                    "br" => append_block(out, ""),
                    _ => {
                        let mut nested = String::new();
                        render_blocks(&element.children, &mut nested);
                        if !nested.trim().is_empty() {
                            append_block(out, nested.trim());
                        }
                    }
                }
            }
        }
        index += 1;
    }
}

fn try_render_multiline_code(p_element: &Element) -> Option<String> {
    let code_el = find_nested_code_element(p_element)?;
    let raw = render_raw_inline_children(&code_el.children);
    if !raw.contains('\n') {
        return None;
    }
    let lines: Vec<&str> = raw.split('\n').map(|l| l.trim()).collect();
    let non_empty: Vec<&str> = lines.iter().copied().filter(|l| !l.is_empty()).collect();
    if non_empty.len() <= 1 {
        return None;
    }
    Some(format!("```\n{}\n```", non_empty.join("\n")))
}

fn find_nested_code_element(element: &Element) -> Option<&Element> {
    for child in &element.children {
        if let Node::Element(el) = child {
            if matches!(el.name.as_str(), "code" | "tt" | "kbd" | "samp") {
                if contains_br(el) {
                    return Some(el);
                }
            }
            if let Some(found) = find_nested_code_element(el) {
                return Some(found);
            }
        }
    }
    None
}

fn contains_br(element: &Element) -> bool {
    for child in &element.children {
        if let Node::Element(el) = child {
            if el.name == "br" {
                return true;
            }
            if contains_br(el) {
                return true;
            }
        }
    }
    false
}

fn render_inline_run(nodes: &[Node], start: usize) -> Option<(usize, String)> {
    let mut index = start;
    let mut raw = String::new();

    while index < nodes.len() {
        match &nodes[index] {
            Node::Text(text) => raw.push_str(&format_plain_text(&decode_html_entities(text))),
            Node::Element(element) if !should_skip_element(element) && is_inline_tag(&element.name) => {
                raw.push_str(&render_inline_element(element));
            }
            _ => break,
        }
        index += 1;
    }

    if index == start {
        None
    } else {
        Some((index, normalize_inline(&raw)))
    }
}

fn render_list(element: &Element, ordered: bool, depth: usize) -> String {
    let mut out = String::new();
    let mut index = 1;

    for child in &element.children {
        let Node::Element(item) = child else {
            continue;
        };
        if item.name != "li" {
            continue;
        }

        let indent = "  ".repeat(depth);
        let marker = if ordered {
            format!("{index}.")
        } else {
            "-".to_string()
        };
        let first_line_prefix = format!("{indent}{marker} ");
        let continuation_prefix = format!("{indent}  ");

        let primary = render_list_item_primary(item);
        if primary.is_empty() {
            out.push_str(first_line_prefix.trim_end());
            out.push('\n');
        } else {
            out.push_str(&first_line_prefix);
            out.push_str(&primary);
            out.push('\n');
        }

        for child in &item.children {
            let Node::Element(nested) = child else {
                continue;
            };

            if is_inline_tag(&nested.name) {
                continue;
            }

            let rendered = match nested.name.as_str() {
                "ul" => render_list(nested, false, depth + 1),
                "ol" => render_list(nested, true, depth + 1),
                "pre" => {
                    let text = render_preformatted(nested);
                    let text = text.trim_matches('\n').to_string();
                    if text.trim().is_empty() {
                        String::new()
                    } else {
                        format!("```\n{text}\n```\n")
                    }
                }
                "table" => render_table(nested),
                "dl" => render_definition_list(nested),
                "blockquote" => {
                    let mut nested_out = String::new();
                    render_blocks(&nested.children, &mut nested_out);
                    prefix_lines(nested_out.trim(), "> ")
                }
                "p" => String::new(),
                _ => {
                    let mut nested_out = String::new();
                    render_blocks(&nested.children, &mut nested_out);
                    nested_out
                }
            };

            if rendered.trim().is_empty() {
                continue;
            }

            for line in rendered.trim_end().lines() {
                if line.is_empty() {
                    out.push('\n');
                } else {
                    out.push_str(&continuation_prefix);
                    out.push_str(line);
                    out.push('\n');
                }
            }
        }

        index += 1;
    }

    out
}

fn render_list_item_primary(item: &Element) -> String {
    let mut raw = String::new();

    for child in &item.children {
        match child {
            Node::Text(text) => raw.push_str(&format_plain_text(&decode_html_entities(text))),
            Node::Element(element) => match element.name.as_str() {
                "ul" | "ol" | "pre" | "table" | "dl" | "blockquote" => {}
                _ if contains_block_descendant(element) => {}
                "p" => {
                    if !raw.trim().is_empty() {
                        raw.push(' ');
                    }
                    raw.push_str(&render_inline_children(&element.children));
                }
                _ => raw.push_str(&render_inline_element(element)),
            },
        }
    }

    normalize_inline(&raw)
}

fn contains_block_descendant(element: &Element) -> bool {
    if matches!(
        element.name.as_str(),
        "ul" | "ol" | "pre" | "table" | "dl" | "blockquote" | "hr"
    ) {
        return true;
    }

    element.children.iter().any(|child| match child {
        Node::Text(_) => false,
        Node::Element(child) => contains_block_descendant(child),
    })
}

fn render_definition_list(element: &Element) -> String {
    let mut out = String::new();
    let mut current_term = String::new();

    for child in &element.children {
        let Node::Element(entry) = child else {
            continue;
        };
        match entry.name.as_str() {
            "dt" => current_term = normalize_inline(&render_inline_children(&entry.children)),
            "dd" => {
                let rendered = render_definition_item(&current_term, entry);
                if rendered.trim().is_empty() {
                    continue;
                }
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(rendered.trim_end());
            }
            _ => {}
        }
    }

    out
}

fn render_definition_item(term: &str, entry: &Element) -> String {
    let (lead, blocks) = split_definition_children(&entry.children);
    let lead = normalize_inline(&lead);

    let mut out = String::new();
    if is_note_term(term) {
        if lead.is_empty() {
            let _ = write!(out, "{term}");
        } else {
            let _ = write!(out, "{term} {lead}");
        }
    } else if term.is_empty() {
        if !lead.is_empty() {
            let _ = write!(out, "- {lead}");
        }
    } else if lead.is_empty() {
        let _ = write!(out, "- {term}");
    } else if term_ends_with_colon(term) {
        let _ = write!(out, "- {term} {lead}");
    } else {
        let _ = write!(out, "- {term}: {lead}");
    }

    for block in blocks {
        if block.trim().is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        let trimmed = block.trim_end();
        if !is_note_term(term) && is_note_block(trimmed) {
            out.push_str(&indent_note_block(trimmed));
        } else {
            let indent = if is_note_term(term) {
                ""
            } else if is_list_like_block(trimmed) {
                "    "
            } else {
                "  "
            };
            out.push_str(&indent_block(trimmed, indent));
        }
    }

    out
}

fn split_definition_children(children: &[Node]) -> (String, Vec<String>) {
    let mut lead = String::new();
    let mut blocks = Vec::new();
    let mut seen_block = false;

    for child in children {
        match child {
            Node::Text(text) if !seen_block => lead.push_str(&format_plain_text(&decode_html_entities(text))),
            Node::Element(element) if !seen_block && is_inline_tag(&element.name) => {
                lead.push_str(&render_inline_element(element));
            }
            Node::Element(element) => {
                seen_block = true;
                match element.name.as_str() {
                    "ul" => blocks.push(render_list(element, false, 0)),
                    "ol" => blocks.push(render_list(element, true, 0)),
                    "pre" => {
                        let text = render_preformatted(element);
                        let text = text.trim_matches('\n');
                        if !text.trim().is_empty() {
                            blocks.push(format!("```\n{text}\n```"));
                        }
                    }
                    "table" => blocks.push(render_table(element)),
                    "dl" => blocks.push(render_definition_list(element)),
                    "blockquote" => {
                        let mut nested = String::new();
                        render_blocks(&element.children, &mut nested);
                        let quoted = prefix_lines(nested.trim(), "> ");
                        if !quoted.trim().is_empty() {
                            blocks.push(quoted);
                        }
                    }
                    "p" => {
                        let text = normalize_inline(&render_inline_children(&element.children));
                        if !text.is_empty() {
                            if blocks.is_empty() {
                                if !lead.is_empty() {
                                    lead.push(' ');
                                }
                                lead.push_str(&text);
                            } else {
                                blocks.push(text);
                            }
                        }
                    }
                    _ if is_inline_tag(&element.name) => {
                        let text = normalize_inline(&render_inline_element(element));
                        if !text.is_empty() {
                            if blocks.is_empty() {
                                if !lead.is_empty() {
                                    lead.push(' ');
                                }
                                lead.push_str(&text);
                            } else if let Some(last) = blocks.last_mut() {
                                if !last.ends_with('\n') && !last.is_empty() {
                                    last.push(' ');
                                }
                                last.push_str(&text);
                            }
                        }
                    }
                    _ => {
                        let mut nested = String::new();
                        render_blocks(&element.children, &mut nested);
                        if !nested.trim().is_empty() {
                            let text = format_plain_text(nested.trim());
                            if blocks.is_empty() && !text.contains('\n') {
                                if !lead.is_empty() {
                                    lead.push(' ');
                                }
                                lead.push_str(&text);
                            } else {
                                blocks.push(text);
                            }
                        }
                    }
                }
            }
            Node::Text(text) => {
                let text = normalize_inline(&format_plain_text(&decode_html_entities(text)));
                if text.is_empty() {
                    continue;
                }
                if blocks.is_empty() {
                    if !lead.is_empty() {
                        lead.push(' ');
                    }
                    lead.push_str(&text);
                } else if let Some(last) = blocks.last_mut() {
                    if !last.ends_with('\n') && !last.is_empty() {
                        last.push(' ');
                    }
                    last.push_str(&text);
                }
            }
        }
    }

    (lead, blocks)
}

fn indent_block(block: &str, prefix: &str) -> String {
    let mut out = String::new();
    for line in block.lines() {
        if line.is_empty() {
            out.push('\n');
        } else {
            out.push_str(prefix);
            out.push_str(line);
            out.push('\n');
        }
    }
    out.trim_end_matches('\n').to_string()
}

fn indent_note_block(block: &str) -> String {
    let mut lines = block.lines();
    let Some(first) = lines.next() else {
        return String::new();
    };

    let mut out = String::new();
    out.push_str("    - ");
    out.push_str(first);
    out.push('\n');

    for line in lines {
        if line.is_empty() {
            out.push('\n');
        } else {
            out.push_str("      ");
            out.push_str(line);
            out.push('\n');
        }
    }

    out.trim_end_matches('\n').to_string()
}

fn render_table(element: &Element) -> String {
    let rows = extract_table_rows(element);
    if rows.is_empty() {
        return String::new();
    }

    let has_header = rows
        .iter()
        .any(|row| row.iter().any(|cell| cell.header));

    if rows.len() == 1 && rows[0].len() > 1 && !has_header {
        if let Some(items) = flatten_simple_word_table(&rows[0]) {
            let mut out = String::new();
            for item in items {
                let _ = writeln!(out, "- {item}");
            }
            return out;
        }

        let mut out = String::new();
        for cell in &rows[0] {
            let text = normalize_inline(&cell.text);
            if !text.is_empty() {
                let _ = writeln!(out, "- {text}");
            }
        }
        return out;
    }

    if !has_header && rows.iter().all(|row| row.len() == 2) {
        let mut out = String::new();
        for row in rows {
            let left = normalize_inline(&row[0].text);
            let right = normalize_inline(&row[1].text);
            if left.is_empty() && right.is_empty() {
                continue;
            }
            let _ = writeln!(out, "- {}: {}", left, right);
        }
        return out;
    }

    if has_header {
        return render_markdown_table(&rows);
    }

    let mut out = String::new();
    for row in rows {
        let cells: Vec<String> = row
            .into_iter()
            .map(|cell| normalize_inline(&cell.text))
            .filter(|cell| !cell.is_empty())
            .collect();
        if cells.is_empty() {
            continue;
        }
        let _ = writeln!(out, "- {}", cells.join(" | "));
    }
    out
}

fn flatten_simple_word_table(row: &[TableCell]) -> Option<Vec<String>> {
    let mut items = Vec::new();

    for cell in row {
        let text = normalize_inline(&cell.text);
        if text.is_empty() {
            continue;
        }

        let (wrapper, inner) = if text.starts_with("**") && text.ends_with("**") && text.len() >= 4 {
            ("**", &text[2..text.len() - 2])
        } else {
            ("", text.as_str())
        };

        let parts: Vec<&str> = inner.split_whitespace().filter(|part| !part.is_empty()).collect();
        if parts.is_empty() {
            continue;
        }

        if !parts.iter().all(|part| is_simple_table_item(part)) {
            return None;
        }

        for part in parts {
            if wrapper.is_empty() {
                items.push(part.to_string());
            } else {
                items.push(format!("{wrapper}{part}{wrapper}"));
            }
        }
    }

    if items.len() > row.len() {
        Some(items)
    } else {
        None
    }
}

fn is_simple_table_item(item: &str) -> bool {
    !item.is_empty()
        && item
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '!' | '{' | '}' | '[' | ']' | '_' | '-' | '+' | '?' | '*' | '/' | '.'))
}

fn render_markdown_table(rows: &[Vec<TableCell>]) -> String {
    let width = rows.iter().map(|row| row.len()).max().unwrap_or(0);
    if width == 0 {
        return String::new();
    }

    let header_index = rows.iter().position(|row| row.iter().any(|cell| cell.header)).unwrap_or(0);
    let header_cells = pad_cells(
        rows[header_index]
            .iter()
            .map(|cell| markdown_table_cell(&normalize_inline(&cell.text)))
            .collect(),
        width,
    );

    let mut out = String::new();
    out.push('|');
    out.push(' ');
    out.push_str(&header_cells.join(" | "));
    out.push_str(" |\n| ");
    out.push_str(&vec!["---"; width].join(" | "));
    out.push_str(" |\n");

    for (index, row) in rows.iter().enumerate() {
        if index == header_index {
            continue;
        }
        let cells = pad_cells(
            row.iter()
                .map(|cell| markdown_table_cell(&normalize_inline(&cell.text)))
                .collect(),
            width,
        );
        out.push('|');
        out.push(' ');
        out.push_str(&cells.join(" | "));
        out.push_str(" |\n");
    }

    out
}

fn render_preformatted(element: &Element) -> String {
    let mut out = String::new();
    collect_pre_text(&element.children, &mut out);
    decode_html_entities(&out)
}

fn render_inline_children(children: &[Node]) -> String {
    let mut out = String::new();
    for child in children {
        match child {
            Node::Text(text) => out.push_str(&format_plain_text(&decode_html_entities(text))),
            Node::Element(element) => out.push_str(&render_inline_element(element)),
        }
    }
    out
}

fn render_inline_element(element: &Element) -> String {
    if should_skip_element(element) {
        return String::new();
    }

    match element.name.as_str() {
        "br" => "\n".to_string(),
        "a" => {
            let text = normalize_inline(&render_inline_children(&element.children));
            if text.is_empty() {
                String::new()
            } else if let Some(href) = attr_value(element, "href") {
                let href = rewrite_posix_href(&decode_html_entities(href));
                if href.starts_with("javascript:") {
                    String::new()
                } else {
                    format!("[{}]({})", text, href)
                }
            } else {
                text
            }
        }
        "b" | "strong" => wrap_markdown("**", &render_inline_children(&element.children)),
        "i" | "em" => wrap_markdown("*", &render_inline_children(&element.children)),
        "tt" | "code" | "kbd" | "samp" => inline_code(&normalize_inline(&render_raw_inline_children(&element.children))),
        "sub" | "sup" | "small" => {
            let text = normalize_inline(&render_inline_children(&element.children));
            if text == "[]" {
                String::new()
            } else {
                text
            }
        }
        "span" | "font" | "basefont" | "center" => render_inline_children(&element.children),
        "p" => render_inline_children(&element.children),
        _ => render_inline_children(&element.children),
    }
}

fn render_raw_inline_children(children: &[Node]) -> String {
    let mut out = String::new();
    for child in children {
        match child {
            Node::Text(text) => out.push_str(&decode_html_entities(text)),
            Node::Element(element) => match element.name.as_str() {
                "br" => out.push('\n'),
                _ => out.push_str(&render_raw_inline_children(&element.children)),
            },
        }
    }
    out
}

fn find_first_element<'a>(element: &'a Element, name: &str) -> Option<&'a Element> {
    if element.name == name {
        return Some(element);
    }
    for child in &element.children {
        if let Node::Element(child_element) = child {
            if let Some(found) = find_first_element(child_element, name) {
                return Some(found);
            }
        }
    }
    None
}

fn extract_heading_anchor(element: &Element) -> Option<String> {
    attr_value(element, "id")
        .or_else(|| attr_value(element, "name"))
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            element.children.iter().find_map(|child| {
                let Node::Element(child_element) = child else {
                    return None;
                };
                if child_element.name != "a" {
                    return None;
                }
                attr_value(child_element, "id")
                    .or_else(|| attr_value(child_element, "name"))
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
            })
        })
}

fn collect_heading_anchor_slugs(document: &Node) -> HashMap<String, String> {
    let mut mappings = HashMap::new();
    let mut slug_counts: HashMap<String, usize> = HashMap::new();
    collect_heading_anchor_slugs_into(document, &mut mappings, &mut slug_counts);
    mappings
}

fn collect_heading_anchor_slugs_into(
    node: &Node,
    mappings: &mut HashMap<String, String>,
    slug_counts: &mut HashMap<String, usize>,
) {
    let Node::Element(element) = node else {
        return;
    };

    if is_heading_tag(&element.name) {
        if let Some(anchor) = extract_heading_anchor(element) {
            let text = normalize_inline(&render_inline_children(&element.children));
            if !text.is_empty() {
                let base_slug = slugify_heading(&text);
                if !base_slug.is_empty() {
                    let count = slug_counts.entry(base_slug.clone()).or_insert(0);
                    let slug = if *count == 0 {
                        base_slug
                    } else {
                        format!("{}-{}", base_slug, *count)
                    };
                    *count += 1;
                    mappings.insert(anchor, slug);
                }
            }
        }
    }

    // Map <a name="tag_..."> anchors that precede a heading to that heading's slug.
    // POSIX HTML commonly places anchors as siblings before headings.
    let mut pending_anchors: Vec<String> = Vec::new();
    for child in &element.children {
        match child {
            Node::Element(child_el) if child_el.name == "a" && !is_heading_tag(&child_el.name) => {
                if let Some(id) = attr_value(child_el, "id")
                    .or_else(|| attr_value(child_el, "name"))
                    .filter(|v| !v.is_empty())
                {
                    pending_anchors.push(id.to_string());
                }
            }
            Node::Element(child_el) if is_heading_tag(&child_el.name) => {
                if !pending_anchors.is_empty() {
                    let text = normalize_inline(&render_inline_children(&child_el.children));
                    if !text.is_empty() {
                        let slug = slugify_heading(&text);
                        // POSIX utility pages place <a name="set"> (and tag_19_*) immediately before an
                        // h4 "NAME" heading. Slugifying that heading yields "name", which would remap
                        // every #set / #tag_19_* link to #name and break cross-references. Skip pending
                        // remapping only for that boilerplate heading.
                        if !slug.is_empty() && slug != "name" {
                            for anchor in pending_anchors.drain(..) {
                                mappings.entry(anchor).or_insert_with(|| slug.clone());
                            }
                        } else {
                            pending_anchors.clear();
                        }
                    }
                }
                pending_anchors.clear();
            }
            Node::Text(t) if t.trim().is_empty() => {}
            _ => pending_anchors.clear(),
        }
    }

    for child in &element.children {
        collect_heading_anchor_slugs_into(child, mappings, slug_counts);
    }
}

fn slugify_heading(text: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = false;

    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            last_was_dash = false;
        } else if ch.is_whitespace() || ch == '-' {
            if !slug.is_empty() && !last_was_dash {
                slug.push('-');
                last_was_dash = true;
            }
        }
    }

    slug.trim_matches('-').to_string()
}

fn heading_slug_for_href(href: &str) -> Option<String> {
    let current_input = CURRENT_INPUT_PATH.get()?;
    let (path_part, anchor) = match href.split_once('#') {
        Some((path, anchor)) => (path, anchor),
        None => ("", ""),
    };
    if anchor.is_empty() {
        return None;
    }

    let target_path = if path_part.is_empty() {
        current_input.clone()
    } else if path_part.ends_with(".html") {
        current_input.parent()?.join(path_part)
    } else {
        return None;
    };

    let cache = HEADING_SLUG_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut cache = cache.lock().ok()?;
    if !cache.contains_key(&target_path) {
        let html = fs::read_to_string(&target_path).ok()?;
        let document = parse_html(&html);
        let mappings = collect_heading_anchor_slugs(&document);
        cache.insert(target_path.clone(), mappings);
    }

    cache.get(&target_path)?.get(anchor).cloned()
}

fn find_first_element_text(node: &Node, name: &str) -> Option<String> {
    match node {
        Node::Text(_) => None,
        Node::Element(element) => {
            if element.name == name {
                return Some(render_inline_children(&element.children));
            }
            for child in &element.children {
                if let Some(found) = find_first_element_text(child, name) {
                    return Some(found);
                }
            }
            None
        }
    }
}

fn trim_to_first_heading(children: &[Node]) -> &[Node] {
    let index = children.iter().position(|child| match child {
        Node::Element(element) => is_heading_tag(&element.name),
        Node::Text(_) => false,
    });
    index.map(|index| &children[index..]).unwrap_or(children)
}

fn should_skip_element(element: &Element) -> bool {
    if matches!(element.name.as_str(), "script" | "style" | "noscript") {
        return true;
    }

    if matches!(element.name.as_str(), "div" | "table" | "tr" | "td") {
        if let Some(class) = attr_value(element, "class") {
            let class = class.to_ascii_lowercase();
            if class.contains("navheader") || class.contains("navfooter") || class == "nav" {
                return true;
            }
        }
        if let Some(summary) = attr_value(element, "summary") {
            if summary.to_ascii_lowercase().contains("navigation") {
                return true;
            }
        }
    }

    false
}

fn extract_table_rows(element: &Element) -> Vec<Vec<TableCell>> {
    let mut rows = Vec::new();
    collect_rows(element, &mut rows);
    rows
}

fn collect_rows(element: &Element, rows: &mut Vec<Vec<TableCell>>) {
    if should_skip_element(element) {
        return;
    }

    if element.name == "tr" {
        let mut cells = Vec::new();
        collect_cells(element, &mut cells);
        if cells.iter().any(|cell| !normalize_inline(&cell.text).is_empty()) {
            rows.push(cells);
        }
        return;
    }

    for child in &element.children {
        if let Node::Element(child_element) = child {
            collect_rows(child_element, rows);
        }
    }
}

fn collect_cells(element: &Element, cells: &mut Vec<TableCell>) {
    for child in &element.children {
        let Node::Element(cell) = child else {
            continue;
        };

        match cell.name.as_str() {
            "td" | "th" => {
                let text = render_inline_children(&cell.children);
                cells.push(TableCell {
                    text,
                    header: cell.name == "th",
                });
            }
            _ => collect_cells(cell, cells),
        }
    }
}

fn collect_pre_text(nodes: &[Node], out: &mut String) {
    for node in nodes {
        match node {
            Node::Text(text) => out.push_str(text),
            Node::Element(element) => match element.name.as_str() {
                "br" => out.push('\n'),
                _ => collect_pre_text(&element.children, out),
            },
        }
    }
}

fn append_block(out: &mut String, block: &str) {
    let block = block.trim();
    if block.is_empty() {
        return;
    }
    if !out.is_empty() {
        if out.ends_with('\n') {
            out.push('\n');
        } else {
            out.push_str("\n\n");
        }
    }
    out.push_str(block);
    if !out.ends_with('\n') {
        out.push('\n');
    }
}

fn push_node(stack: &mut Vec<OpenElement>, node: Node) {
    if let Some(parent) = stack.last_mut() {
        parent.children.push(node);
    }
}

fn push_text(stack: &mut Vec<OpenElement>, text: &str) {
    if !text.is_empty() {
        push_node(stack, Node::Text(text.to_string()));
    }
}

fn pop_open_element(stack: &mut Vec<OpenElement>) {
    if stack.len() <= 1 {
        return;
    }
    let open = stack.pop().unwrap();
    push_node(
        stack,
        Node::Element(Element {
            name: open.name,
            attrs: open.attrs,
            children: open.children,
        }),
    );
}

fn close_to(stack: &mut Vec<OpenElement>, target: &str) {
    if let Some(position) = stack.iter().rposition(|element| element.name == target) {
        while stack.len() > position {
            pop_open_element(stack);
        }
    }
}

fn implicitly_close_for_start(stack: &mut Vec<OpenElement>, name: &str) {
    loop {
        let top = match stack.last() {
            Some(top) if stack.len() > 1 => top.name.as_str(),
            _ => break,
        };

        let should_close = match (top, name) {
            ("p", next) if closes_paragraph(next) => true,
            ("li", "li") => true,
            ("dt", "dt" | "dd") => true,
            ("dd", "dt" | "dd") => true,
            ("tr", "tr") => true,
            ("td", "td" | "th" | "tr") => true,
            ("th", "td" | "th" | "tr") => true,
            _ => false,
        };

        if should_close {
            pop_open_element(stack);
        } else {
            break;
        }
    }
}

fn closes_paragraph(name: &str) -> bool {
    matches!(
        name,
        "address"
            | "article"
            | "aside"
            | "blockquote"
            | "center"
            | "details"
            | "div"
            | "dl"
            | "fieldset"
            | "figcaption"
            | "figure"
            | "footer"
            | "form"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "header"
            | "hr"
            | "menu"
            | "nav"
            | "ol"
            | "p"
            | "pre"
            | "section"
            | "table"
            | "ul"
    )
}

fn find_tag_end(input: &str, mut index: usize) -> Option<usize> {
    let bytes = input.as_bytes();
    let mut quote = None;

    while index < input.len() {
        let byte = bytes[index];
        match quote {
            Some(current) if byte == current => quote = None,
            Some(_) => {}
            None if byte == b'"' || byte == b'\'' => quote = Some(byte),
            None if byte == b'>' => return Some(index),
            None => {}
        }
        index += 1;
    }

    None
}

fn find_case_insensitive_closing_tag(input: &str, lower: &str, start: usize, name: &str) -> Option<usize> {
    let needle = format!("</{name}");
    let offset = lower[start..].find(&needle)?;
    let close_start = start + offset + 2;
    find_tag_end(input, close_start).map(|end| end + 1)
}

fn parse_start_tag(raw: &str) -> Option<ParsedTag> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }

    let mut chars = raw.char_indices();
    let mut end_of_name = 0;
    for (index, ch) in chars.by_ref() {
        if ch.is_whitespace() || ch == '/' {
            end_of_name = index;
            break;
        }
    }
    if end_of_name == 0 {
        end_of_name = raw.len();
    }

    let name = parse_tag_name(&raw[..end_of_name]);
    if name.is_empty() {
        return None;
    }

    let attrs_raw = &raw[end_of_name..];
    let self_closing = raw.trim_end().ends_with('/') || is_void_tag(&name);
    let attrs = parse_attributes(attrs_raw);

    Some(ParsedTag {
        name,
        attrs,
        self_closing,
    })
}

fn parse_tag_name(raw: &str) -> String {
    raw.chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || matches!(ch, ':' | '_' | '-'))
        .collect::<String>()
        .to_ascii_lowercase()
}

fn parse_attributes(raw: &str) -> Vec<(String, String)> {
    let bytes = raw.as_bytes();
    let mut attrs = Vec::new();
    let mut i = 0;

    while i < raw.len() {
        while i < raw.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= raw.len() || bytes[i] == b'/' {
            break;
        }

        let start = i;
        while i < raw.len() {
            let byte = bytes[i];
            if byte.is_ascii_whitespace() || byte == b'=' || byte == b'/' {
                break;
            }
            i += 1;
        }

        let key = raw[start..i].trim().to_ascii_lowercase();
        if key.is_empty() {
            i += 1;
            continue;
        }

        while i < raw.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }

        let mut value = String::new();
        if i < raw.len() && bytes[i] == b'=' {
            i += 1;
            while i < raw.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }

            if i < raw.len() && (bytes[i] == b'"' || bytes[i] == b'\'') {
                let quote = bytes[i];
                i += 1;
                let start = i;
                while i < raw.len() && bytes[i] != quote {
                    i += 1;
                }
                value.push_str(&raw[start..i.min(raw.len())]);
                if i < raw.len() {
                    i += 1;
                }
            } else {
                let start = i;
                while i < raw.len() && !bytes[i].is_ascii_whitespace() && bytes[i] != b'/' {
                    i += 1;
                }
                value.push_str(&raw[start..i]);
            }
        }

        attrs.push((key, value));
    }

    attrs
}

fn attr_value<'a>(element: &'a Element, key: &str) -> Option<&'a str> {
    element
        .attrs
        .iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.as_str())
}

fn decode_html_entities(input: &str) -> String {
    let mut out = String::new();
    let mut chars = input.char_indices().peekable();

    while let Some((index, ch)) = chars.next() {
        if ch != '&' {
            out.push(ch);
            continue;
        }

        let mut end = None;
        let mut probe = chars.clone();
        while let Some((probe_index, probe_ch)) = probe.next() {
            if probe_ch == ';' {
                end = Some(probe_index);
                break;
            }
            if probe_ch.is_whitespace() || probe_index - index > 16 {
                break;
            }
        }

        let Some(end_index) = end else {
            out.push('&');
            continue;
        };

        let entity = &input[index + 1..end_index];
        if let Some(decoded) = decode_entity(entity) {
            out.push(decoded);
            while let Some((probe_index, _)) = chars.peek() {
                if *probe_index <= end_index {
                    chars.next();
                } else {
                    break;
                }
            }
        } else {
            out.push('&');
        }
    }

    out
}

fn decode_entity(entity: &str) -> Option<char> {
    match entity {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        "nbsp" => Some(' '),
        "copy" => Some('©'),
        "reg" => Some('®'),
        "sect" => Some('§'),
        "para" => Some('¶'),
        "mdash" => Some('-'),
        "ndash" => Some('-'),
        _ if entity.starts_with("#x") || entity.starts_with("#X") => {
            u32::from_str_radix(&entity[2..], 16).ok().and_then(char::from_u32)
        }
        _ if entity.starts_with('#') => entity[1..].parse::<u32>().ok().and_then(char::from_u32),
        _ => None,
    }
}

fn normalize_inline(input: &str) -> String {
    let mut out = String::new();
    let mut pending_space = false;

    for ch in input.chars() {
        if ch.is_whitespace() {
            pending_space = true;
            continue;
        }

        if pending_space && !out.is_empty() && !out.ends_with('\n') {
            out.push(' ');
        }

        pending_space = false;
        out.push(ch);
    }

    out.trim().to_string()
}

fn inline_code(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let mut max_run = 0usize;
    let mut current_run = 0usize;
    for ch in trimmed.chars() {
        if ch == '`' {
            current_run += 1;
            max_run = max_run.max(current_run);
        } else {
            current_run = 0;
        }
    }

    let fence = "`".repeat(max_run + 1);
    if trimmed.starts_with('`') || trimmed.ends_with('`') {
        format!("{fence} {trimmed} {fence}")
    } else {
        format!("{fence}{trimmed}{fence}")
    }
}

fn wrap_markdown(wrapper: &str, text: &str) -> String {
    let text = normalize_inline(text);
    if text.is_empty() {
        String::new()
    } else {
        format!("{wrapper}{text}{wrapper}")
    }
}

fn format_plain_text(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.char_indices().peekable();

    while let Some((index, ch)) = chars.next() {
        if ch != '<' {
            if ch == '>' {
                out.push_str("\\>");
            } else {
                out.push(ch);
            }
            continue;
        }

        let mut end = None;
        let mut probe = chars.clone();
        while let Some((probe_index, probe_ch)) = probe.next() {
            if probe_ch == '>' {
                end = Some(probe_index);
                break;
            }
            if probe_ch == '<' || probe_ch == '\n' || probe_ch == '\r' || probe_index - index > 64 {
                break;
            }
        }

        let Some(end_index) = end else {
            out.push_str("\\<");
            continue;
        };

        let inner = &text[index + 1..end_index];
        if looks_like_posix_metasyntax(inner) {
            out.push('`');
            out.push('<');
            out.push_str(inner);
            out.push('>');
            out.push('`');
            while let Some((probe_index, _)) = chars.peek() {
                if *probe_index <= end_index {
                    chars.next();
                } else {
                    break;
                }
            }
        } else {
            out.push_str("\\<");
        }
    }

    out
}

fn looks_like_posix_metasyntax(inner: &str) -> bool {
    let inner = inner.trim();
    !inner.is_empty()
        && inner
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, ' ' | '-' | '_' | '/' | '+' | '\'' | '"' | ':'))
}

fn term_ends_with_colon(term: &str) -> bool {
    term.trim_end_matches('*')
        .trim_end_matches('`')
        .trim_end()
        .ends_with(':')
}

fn is_note_term(term: &str) -> bool {
    matches!(term.trim(), "Note:" | "**Note:**")
}

fn is_note_block(block: &str) -> bool {
    matches!(block.trim_start(), s if s.starts_with("**Note:**") || s.starts_with("Note:"))
}

fn is_list_like_block(block: &str) -> bool {
    let trimmed = block.trim_start();
    if trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("+ ") {
        return true;
    }

    let mut seen_digit = false;
    for ch in trimmed.chars() {
        if ch.is_ascii_digit() {
            seen_digit = true;
            continue;
        }
        return seen_digit && ch == '.';
    }
    false
}

fn rewrite_posix_href(href: &str) -> String {
    let (path, anchor) = match href.split_once('#') {
        Some((p, a)) => (p, Some(a)),
        None => (href, None),
    };

    let rewritten_anchor = if let Some(a) = anchor {
        if a.is_empty() {
            None
        } else {
            Some(heading_slug_for_href(href).unwrap_or_else(|| a.to_string()))
        }
    } else {
        None
    };

    let rewritten_path = if path.is_empty() {
        String::new()
    } else {
        let md_path = if path.ends_with(".html") {
            format!("{}.md", &path[..path.len() - 5])
        } else {
            path.to_string()
        };
        root_relative_path(&md_path)
    };

    match (rewritten_path.is_empty(), rewritten_anchor) {
        (true, None) => String::new(),
        (true, Some(a)) => format!("#{a}"),
        (false, None) => rewritten_path,
        (false, Some(a)) => format!("{rewritten_path}#{a}"),
    }
}

fn root_relative_path(relative_md_path: &str) -> String {
    if !relative_md_path.contains("../") {
        return relative_md_path.to_string();
    }

    let Some(input_path) = CURRENT_INPUT_PATH.get() else {
        return relative_md_path.to_string();
    };
    let Some(input_dir) = input_path.parent() else {
        return relative_md_path.to_string();
    };

    let resolved = input_dir.join(relative_md_path);
    let normalized = normalize_path(&resolved);
    let normalized_str = normalized.to_string_lossy();

    if let Some(pos) = find_doc_root(&normalized_str) {
        let within_tree = &normalized_str[pos..];
        return format!("docs/posix/md/{within_tree}");
    }

    relative_md_path.to_string()
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                if !components.is_empty() {
                    components.pop();
                }
            }
            std::path::Component::CurDir => {}
            other => components.push(other),
        }
    }
    components.iter().collect()
}

fn find_doc_root(path: &str) -> Option<usize> {
    for marker in ["susv5-html/", "posix/md/"] {
        if let Some(pos) = path.find(marker) {
            return Some(pos + marker.len());
        }
    }
    None
}

fn heading_level(name: &str) -> usize {
    match name {
        "h1" => 1,
        "h2" => 2,
        "h3" => 3,
        "h4" => 4,
        "h5" => 5,
        "h6" => 6,
        _ => 1,
    }
}

fn is_heading_tag(name: &str) -> bool {
    matches!(name, "h1" | "h2" | "h3" | "h4" | "h5" | "h6")
}

fn is_inline_tag(name: &str) -> bool {
    matches!(
        name,
        "a" | "abbr" | "b" | "code" | "em" | "font" | "i" | "img" | "kbd" | "q" | "samp" | "small" | "span"
            | "strong" | "sub" | "sup" | "tt"
    )
}

fn is_void_tag(name: &str) -> bool {
    matches!(
        name,
        "area"
            | "base"
            | "basefont"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

fn markdown_table_cell(text: &str) -> String {
    text.replace('|', "\\|").replace('\n', " ")
}

fn pad_cells(mut cells: Vec<String>, width: usize) -> Vec<String> {
    while cells.len() < width {
        cells.push(String::new());
    }
    cells
}

fn prefix_lines(input: &str, prefix: &str) -> String {
    let mut out = String::new();
    for line in input.lines() {
        if line.is_empty() {
            out.push_str(prefix.trim_end());
        } else {
            out.push_str(prefix);
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}

fn tidy_markdown(input: String) -> String {
    let mut out = String::new();
    let mut blank_count = 0;
    for line in input.lines() {
        let trimmed = line.trim();
        if is_nav_line(trimmed) {
            continue;
        }
        if trimmed.is_empty() {
            blank_count += 1;
            if blank_count <= 1 {
                out.push('\n');
            }
        } else {
            blank_count = 0;
            out.push_str(line.trim_end());
            out.push('\n');
        }
    }
    let trimmed = strip_trailing_nav(&out);
    trimmed.trim().to_string() + "\n"
}

fn is_nav_line(line: &str) -> bool {
    line.contains("return to top of page")
        || line.contains("registered Trademark of The Open Group")
}

fn strip_trailing_nav(input: &str) -> String {
    let lines: Vec<&str> = input.lines().collect();
    let mut end = lines.len();
    while end > 0 && lines[end - 1].trim().is_empty() {
        end -= 1;
    }
    while end > 0 {
        let line = lines[end - 1].trim();
        if line == "---"
            || line.contains("return to top of page")
            || line.contains("registered Trademark of The Open Group")
        {
            end -= 1;
            while end > 0 && lines[end - 1].trim().is_empty() {
                end -= 1;
            }
        } else {
            break;
        }
    }
    let mut out = String::new();
    for line in &lines[..end] {
        out.push_str(line);
        out.push('\n');
    }
    out
}

#[derive(Clone, Debug)]
struct TableCell {
    text: String,
    header: bool,
}
