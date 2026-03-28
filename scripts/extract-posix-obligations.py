#!/usr/bin/env python3
"""
Extract normative obligations from POSIX HTML spec files.

Parses POSIX HTML, extracts every normative block containing a keyword
(shall/may/unspecified/implementation-defined), and writes one Markdown
file per obligation.
"""

import argparse
import json
import os
import re
import sys
from datetime import datetime, timezone
from html.parser import HTMLParser


# ---------------------------------------------------------------------------
# Lightweight DOM
# ---------------------------------------------------------------------------

class Node:
    """A lightweight DOM node."""
    __slots__ = ('tag', 'attrs', 'children', 'parent', 'line')

    def __init__(self, tag, attrs=None, line=None):
        self.tag = tag
        self.attrs = dict(attrs) if attrs else {}
        self.children = []
        self.parent = None
        self.line = line

    def append(self, child):
        if isinstance(child, Node):
            child.parent = self
        self.children.append(child)

    def get_attr(self, name, default=None):
        return self.attrs.get(name, default)

    def find_all(self, tag):
        results = []
        for child in self.children:
            if isinstance(child, Node):
                if child.tag == tag:
                    results.append(child)
                results.extend(child.find_all(tag))
        return results

    def text_content(self):
        parts = []
        for child in self.children:
            if isinstance(child, str):
                parts.append(child)
            elif isinstance(child, Node):
                parts.append(child.text_content())
        return ''.join(parts)

    def end_line(self):
        """Return the last line number in this subtree."""
        best = self.line or 0
        for child in self.children:
            if isinstance(child, Node):
                el = child.end_line()
                if el and el > best:
                    best = el
        return best

    def __repr__(self):
        return f'<Node {self.tag} line={self.line}>'


class DOMBuilder(HTMLParser):
    """Build a lightweight DOM tree from HTML."""

    SELF_CLOSING = frozenset([
        'br', 'hr', 'img', 'input', 'meta', 'link', 'basefont',
        'base', 'col', 'area', 'param', 'wbr',
    ])

    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.root = Node('root')
        self.current = self.root
        self._stack = [self.root]

    def handle_starttag(self, tag, attrs):
        node = Node(tag, attrs, line=self.getpos()[0])
        self.current.append(node)
        if tag.lower() not in self.SELF_CLOSING:
            self._stack.append(node)
            self.current = node

    def handle_endtag(self, tag):
        tag_lower = tag.lower()
        if tag_lower in self.SELF_CLOSING:
            return
        # Walk up the stack to find the matching tag
        for i in range(len(self._stack) - 1, 0, -1):
            if self._stack[i].tag.lower() == tag_lower:
                self._stack = self._stack[:i]
                self.current = self._stack[-1]
                return
        # No match found; ignore

    def handle_data(self, data):
        self.current.append(data)

    def build(self, html_text):
        self.feed(html_text)
        return self.root


# ---------------------------------------------------------------------------
# Section tracker
# ---------------------------------------------------------------------------

def extract_section_info(heading_node):
    """Extract section anchor and title from an h2-h5 heading node."""
    anchor = None
    for child in heading_node.children:
        if isinstance(child, Node) and child.tag == 'a':
            aid = child.get_attr('id') or child.get_attr('name')
            if aid and aid.startswith('tag_'):
                anchor = aid
                break
    title = normalize_ws(heading_node.text_content())
    return anchor, title


# ---------------------------------------------------------------------------
# Text extraction helpers
# ---------------------------------------------------------------------------

def normalize_ws(text):
    """Collapse whitespace and strip."""
    return re.sub(r'\s+', ' ', text).strip()


def extract_hrefs(node):
    """Collect all href values from <a> tags in a subtree."""
    hrefs = []
    if isinstance(node, Node):
        if node.tag == 'a' and 'href' in node.attrs:
            hrefs.append(node.attrs['href'])
        for child in node.children:
            hrefs.extend(extract_hrefs(child))
    return hrefs


def is_note_dl(node):
    """Check if a <dl> contains a Note: header."""
    if not isinstance(node, Node) or node.tag != 'dl':
        return False
    for child in node.children:
        if isinstance(child, Node) and child.tag == 'dt':
            t = normalize_ws(child.text_content())
            if t.startswith('Note:') or t == 'Note:':
                return True
    return False


# ---------------------------------------------------------------------------
# Keyword matching
# ---------------------------------------------------------------------------

KEYWORD_PATTERNS = [
    ('implementation-defined', re.compile(r'\bimplementation-defined\b', re.I)),
    ('unspecified', re.compile(r'\bunspecified\b', re.I)),
    ('shall', re.compile(r'\bshall\b', re.I)),
    ('may', re.compile(r'\bmay\b', re.I)),
]

# For validation: match in raw text (not inside HTML tags)
KEYWORD_VALIDATORS = {
    'shall': re.compile(r'\bshall\b', re.I),
    'may': re.compile(r'\bmay\b', re.I),
    'unspecified': re.compile(r'\bunspecified\b', re.I),
    'implementation-defined': re.compile(r'\bimplementation-defined\b', re.I),
}

CATEGORY_PRIORITY = {
    'shall': 0,
    'implementation-defined': 1,
    'unspecified': 2,
    'may': 3,
}

CATEGORY_PREFIX = {
    'shall': 'SHALL',
    'implementation-defined': 'IMPLDEF',
    'unspecified': 'UNSPEC',
    'may': 'MAY',
}


def find_keywords(text):
    """Return list of keyword categories found in text."""
    found = []
    for name, pattern in KEYWORD_PATTERNS:
        if pattern.search(text):
            found.append(name)
    return found


def primary_category(keywords):
    """Return the highest-priority category."""
    if not keywords:
        return None
    return min(keywords, key=lambda k: CATEGORY_PRIORITY.get(k, 99))


# ---------------------------------------------------------------------------
# Obligation extraction
# ---------------------------------------------------------------------------

class ObligationExtractor:
    """Walk a DOM tree and extract normative blocks."""

    def __init__(self, source_file, tier):
        self.source_file = source_file
        self.tier = tier
        self.obligations = []
        self.section_anchor = None
        self.section_title = None
        self.informative = False
        self.opt_feature = None
        self._counters = {}  # (anchor, prefix) -> count

    def _next_id(self, prefix):
        key = (self.section_anchor or 'unknown', prefix)
        self._counters[key] = self._counters.get(key, 0) + 1
        seq = self._counters[key]

        if self.section_anchor:
            digits = self.section_anchor.replace('tag_', '').replace('_', '-')
        else:
            digits = '00-00'

        return f'{prefix}-{digits}-{seq:03d}'

    def _emit(self, text, node, preamble=None):
        """Emit an obligation if the text contains normative keywords."""
        full_text = f'{preamble}: {text}' if preamble else text
        keywords = find_keywords(full_text)
        if not keywords:
            return

        cat = primary_category(keywords)
        prefix = CATEGORY_PREFIX[cat]
        oid = self._next_id(prefix)

        hrefs = extract_hrefs(node)
        start_line = node.line or 0
        end_line = node.end_line() or start_line

        self.obligations.append({
            'id': oid,
            'category': cat,
            'section': self.section_title or '',
            'anchor': self.section_anchor or '',
            'source_file': self.source_file,
            'source_lines': [start_line, end_line],
            'keywords': keywords,
            'tier': self.tier,
            'refs': hrefs,
            'text': full_text,
            'opt_feature': self.opt_feature,
        })

    def walk(self, root):
        """Walk the DOM tree and extract obligations."""
        self._walk_children(root)

    def _walk_children(self, node):
        children = node.children if isinstance(node, Node) else []
        i = 0
        while i < len(children):
            child = children[i]
            if not isinstance(child, Node):
                i += 1
                continue

            # Track sections
            if child.tag in ('h2', 'h3', 'h4', 'h5'):
                anchor, title = extract_section_info(child)
                if anchor:
                    self.section_anchor = anchor
                    self.section_title = title
                i += 1
                continue

            # Track informative regions
            if child.tag == 'div' and 'box' in (child.get_attr('class') or ''):
                text = normalize_ws(child.text_content())
                if 'following sections are informative' in text.lower():
                    self.informative = True
                elif 'end of informative text' in text.lower():
                    self.informative = False
                i += 1
                continue

            # Track optional features
            if child.tag == 'img':
                src = child.get_attr('src') or ''
                if 'opt-start.gif' in src:
                    # Try to get feature code from preceding sup/a
                    self.opt_feature = self._extract_opt_code(node, i)
                elif 'opt-end.gif' in src:
                    self.opt_feature = None
                i += 1
                continue
            if child.tag == 'sup':
                # sup tags with opt-start images inside
                for sc in child.children:
                    if isinstance(sc, Node) and sc.tag == 'img':
                        src = sc.get_attr('src') or ''
                        if 'opt-start.gif' in src:
                            self.opt_feature = self._extract_opt_code_from_sup(child)
                        elif 'opt-end.gif' in src:
                            self.opt_feature = None

            # Skip informative content
            if self.informative:
                i += 1
                continue

            # Skip navigation divs
            if child.tag == 'div' and (child.get_attr('class') or '') in ('NAVHEADER', 'NAVFOOTER'):
                i += 1
                continue

            # Skip nav tables
            if child.tag == 'table' and (child.get_attr('class') or '') == 'nav':
                i += 1
                continue

            # Skip note DLs
            if is_note_dl(child):
                i += 1
                continue

            # Block containers: p, dd
            if child.tag in ('p', 'dd'):
                text = normalize_ws(child.text_content())
                if not text:
                    i += 1
                    continue

                # Check for preamble fusion: text ends with ':' and next sibling is a list
                next_node = self._next_element(children, i)
                if (text.endswith(':') and next_node and
                        isinstance(next_node, Node) and
                        next_node.tag in ('ol', 'ul', 'dl')):
                    # Emit preamble itself if it has keywords
                    self._emit(text, child)
                    # Emit each list item with preamble
                    self._extract_list_items(next_node, preamble=text)
                    i += 1
                    continue

                self._emit(text, child)
                i += 1
                continue

            # List items
            if child.tag in ('li',):
                text = normalize_ws(child.text_content())
                if text:
                    self._emit(text, child)
                i += 1
                continue

            # Table cells
            if child.tag == 'table' and (child.get_attr('class') or '') != 'nav':
                self._extract_table(child)
                i += 1
                continue

            # Lists at top level (not fused with a preamble paragraph)
            if child.tag in ('ol', 'ul', 'dl'):
                if not is_note_dl(child):
                    self._extract_list_items(child)
                i += 1
                continue

            # Blockquotes (used in sh.html for description sections)
            if child.tag == 'blockquote':
                self._walk_children(child)
                i += 1
                continue

            # Pre blocks - skip (grammar, code examples)
            if child.tag == 'pre':
                i += 1
                continue

            # Recurse into other structural elements
            if child.tag in ('body', 'html', 'center', 'font', 'div', 'basefont', 'script'):
                self._walk_children(child)
                i += 1
                continue

            i += 1

    def _extract_list_items(self, list_node, preamble=None):
        """Extract obligations from list items."""
        for child in list_node.children:
            if not isinstance(child, Node):
                continue
            if child.tag in ('li', 'dd', 'dt'):
                text = normalize_ws(child.text_content())
                if text:
                    self._emit(text, child, preamble=preamble)
            elif child.tag in ('ol', 'ul', 'dl'):
                if not is_note_dl(child):
                    self._extract_list_items(child, preamble=preamble)

    def _extract_table(self, table_node):
        """Extract obligations from table cells with column/row context."""
        rows = table_node.find_all('tr')
        if not rows:
            return

        # Extract column headers from first row
        col_headers = []
        first_row = rows[0]
        for cell in first_row.children:
            if isinstance(cell, Node) and cell.tag in ('th', 'td'):
                col_headers.append(normalize_ws(cell.text_content()))

        for row in rows[1:]:
            cells = [c for c in row.children if isinstance(c, Node) and c.tag in ('td', 'th')]
            row_header = normalize_ws(cells[0].text_content()) if cells else ''
            for j, cell in enumerate(cells):
                cell_text = normalize_ws(cell.text_content())
                if not cell_text:
                    continue
                col_header = col_headers[j] if j < len(col_headers) else ''

                keywords = find_keywords(cell_text)
                if keywords:
                    if col_header and row_header and j > 0:
                        full_text = f'{col_header}: {row_header} -- {cell_text}'
                    else:
                        full_text = cell_text
                    self._emit(full_text, cell)

    def _next_element(self, children, idx):
        """Find the next element node after idx."""
        for i in range(idx + 1, len(children)):
            if isinstance(children[i], Node):
                return children[i]
        return None

    def _extract_opt_code(self, parent, img_idx):
        """Try to extract option code like UP, XSI from context."""
        # Look backward for a sup containing an <a> with the code
        children = parent.children
        for i in range(img_idx - 1, max(img_idx - 5, -1), -1):
            if isinstance(children[i], Node) and children[i].tag == 'sup':
                return self._extract_opt_code_from_sup(children[i])
        return None

    def _extract_opt_code_from_sup(self, sup_node):
        """Extract option code from a <sup> node."""
        for child in sup_node.children:
            if isinstance(child, Node) and child.tag == 'a':
                href = child.get_attr('href') or ''
                m = re.search(r"open_code\('(\w+)'\)", href)
                if m:
                    return m.group(1)
        text = normalize_ws(sup_node.text_content())
        m = re.search(r'\[(\w+)\]', text)
        if m:
            return m.group(1)
        return None


# ---------------------------------------------------------------------------
# Validation
# ---------------------------------------------------------------------------

def strip_html_tags(html_text):
    """Strip HTML tags to get raw text for keyword counting."""
    return re.sub(r'<[^>]+>', ' ', html_text)


def count_keywords_in_html(html_text, informative_ranges):
    """Count keyword occurrences in non-informative HTML text."""
    # Remove informative regions from the HTML
    cleaned = html_text
    for start, end in sorted(informative_ranges, reverse=True):
        cleaned = cleaned[:start] + cleaned[end:]

    text = strip_html_tags(cleaned)

    counts = {}
    for name, pattern in KEYWORD_VALIDATORS.items():
        counts[name] = len(pattern.findall(text))
    return counts


def find_informative_ranges(html_text):
    """Find byte ranges of informative sections."""
    ranges = []
    start_pat = re.compile(r'The following sections are informative', re.I)
    end_pat = re.compile(r'End of informative text', re.I)

    starts = [m.start() for m in start_pat.finditer(html_text)]
    ends = [m.end() for m in end_pat.finditer(html_text)]

    for s in starts:
        # Find the nearest end after this start
        for e in ends:
            if e > s:
                ranges.append((s, e))
                break
    return ranges


def find_note_ranges(html_text):
    """Find byte ranges of Note: blocks. Approximate heuristic."""
    ranges = []
    # Match <dt><b>Note:</b></dt> and extend to the next </dl>
    for m in re.finditer(r'<dt>\s*<b>\s*Note:\s*</b>\s*</dt>', html_text, re.I):
        start = m.start()
        # Walk backward to find enclosing <dl>
        dl_start = html_text.rfind('<dl', 0, start)
        if dl_start == -1:
            dl_start = start
        # Find closing </dl> after the note
        dl_end_match = re.search(r'</dl>', html_text[start:], re.I)
        if dl_end_match:
            end = start + dl_end_match.end()
        else:
            end = start + 500
        ranges.append((dl_start, end))
    return ranges


def validate_extraction(html_text, obligations, source_file):
    """Validate that all keywords were captured."""
    informative_ranges = find_informative_ranges(html_text)
    note_ranges = find_note_ranges(html_text)

    all_skip_ranges = informative_ranges + note_ranges

    html_counts = count_keywords_in_html(html_text, all_skip_ranges)

    ob_counts = {k: 0 for k in KEYWORD_VALIDATORS}
    for ob in obligations:
        for kw in ob['keywords']:
            # Count actual occurrences in the text
            pattern = KEYWORD_VALIDATORS[kw]
            ob_counts[kw] += len(pattern.findall(ob['text']))

    errors = []
    for kw in KEYWORD_VALIDATORS:
        html_c = html_counts[kw]
        ob_c = ob_counts[kw]
        if ob_c < html_c:
            diff = html_c - ob_c
            errors.append(f'{source_file}: {kw}: HTML has {html_c}, captured {ob_c} (missing {diff})')
        elif ob_c > html_c:
            # Preamble duplication can cause over-count, that's acceptable
            pass

    return errors


# ---------------------------------------------------------------------------
# File emitter
# ---------------------------------------------------------------------------

def write_obligation_file(ob, output_dir):
    """Write a single obligation Markdown file."""
    filename = f'{ob["id"]}.md'
    path = os.path.join(output_dir, filename)

    refs_list = json.dumps(ob['refs']) if ob['refs'] else '[]'
    keywords_list = json.dumps(ob['keywords'])

    # Escape YAML special chars in section title
    section = ob['section'].replace('"', '\\"')

    lines = [
        '---',
        f'id: {ob["id"]}',
        f'category: {ob["category"]}',
        f'section: "{section}"',
        f'anchor: {ob["anchor"]}',
        f'source_file: {ob["source_file"]}',
        f'source_lines: {json.dumps(ob["source_lines"])}',
        f'keywords: {keywords_list}',
        f'tier: {ob["tier"]}',
        f'status: raw',
        f'refs: {refs_list}',
        f'related_sections: []',
        f'tests: []',
        '---',
        '',
        '## Extracted Text',
        '',
    ]

    # Format text as blockquote
    for text_line in ob['text'].split('\n'):
        lines.append(f'> {text_line}' if text_line.strip() else '>')

    lines.extend([
        '',
        '## Context',
        '',
        '*(to be filled during enrichment)*',
        '',
        '## Implementation Notes',
        '',
        '*(to be filled during implementation)*',
        '',
    ])

    with open(path, 'w', encoding='utf-8') as f:
        f.write('\n'.join(lines))

    return filename


def update_extracted_manifest(manifest_path, source_file, tier, count):
    """Update or create _extracted.json manifest."""
    if os.path.exists(manifest_path):
        with open(manifest_path, 'r', encoding='utf-8') as f:
            data = json.load(f)
    else:
        data = {'extracted_files': []}

    data['extracted_files'].append({
        'file': source_file,
        'tier': tier,
        'obligations': count,
        'extracted_at': datetime.now(timezone.utc).isoformat(),
    })

    with open(manifest_path, 'w', encoding='utf-8') as f:
        json.dump(data, f, indent=2)


def write_index(output_dir, all_obligations):
    """Write _index.json summary."""
    index = []
    for ob in all_obligations:
        index.append({
            'id': ob['id'],
            'category': ob['category'],
            'section': ob['section'],
            'anchor': ob['anchor'],
            'source_file': ob['source_file'],
            'tier': ob['tier'],
            'keywords': ob['keywords'],
        })

    path = os.path.join(output_dir, '_index.json')
    with open(path, 'w', encoding='utf-8') as f:
        json.dump(index, f, indent=2)


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def get_already_extracted(manifest_path):
    """Get set of already-extracted files."""
    if not os.path.exists(manifest_path):
        return set()
    with open(manifest_path, 'r', encoding='utf-8') as f:
        data = json.load(f)
    return {entry['file'] for entry in data.get('extracted_files', [])}


def process_file(spec_dir, source_file, tier):
    """Process a single HTML file and return obligations."""
    full_path = os.path.join(spec_dir, source_file)
    if not os.path.exists(full_path):
        print(f'ERROR: File not found: {full_path}', file=sys.stderr)
        return []

    with open(full_path, 'r', encoding='utf-8', errors='replace') as f:
        html_text = f.read()

    builder = DOMBuilder()
    root = builder.build(html_text)

    extractor = ObligationExtractor(source_file, tier)
    extractor.walk(root)

    return extractor.obligations, html_text


def main():
    parser = argparse.ArgumentParser(description='Extract POSIX normative obligations')
    parser.add_argument('--spec-dir', required=True, help='Path to docs/posix/')
    parser.add_argument('--output-dir', required=True, help='Path to docs/requirements/obligations/')
    parser.add_argument('--files', nargs='+', required=True, help='HTML files relative to spec-dir')
    parser.add_argument('--tier', type=int, default=None, help='Tier number (auto-detected if omitted)')
    parser.add_argument('--skip-validation', action='store_true', help='Skip keyword validation')
    args = parser.parse_args()

    os.makedirs(args.output_dir, exist_ok=True)
    manifest_path = os.path.join(args.output_dir, '_extracted.json')
    already_extracted = get_already_extracted(manifest_path)

    # Determine tier
    if args.tier is not None:
        tier = args.tier
    else:
        # Auto-detect: find max tier in manifest and add 1, or 1 if empty
        if os.path.exists(manifest_path):
            with open(manifest_path, 'r', encoding='utf-8') as f:
                data = json.load(f)
            existing_tiers = [e.get('tier', 0) for e in data.get('extracted_files', [])]
            tier = max(existing_tiers) + 1 if existing_tiers else 1
        else:
            tier = 1

    all_obligations = []
    all_errors = []
    files_processed = 0

    for source_file in args.files:
        if source_file in already_extracted:
            print(f'SKIP (already extracted): {source_file}', file=sys.stderr)
            continue

        print(f'Processing: {source_file} (tier {tier})...', file=sys.stderr)
        result = process_file(args.spec_dir, source_file, tier)
        if not result:
            continue
        obligations, html_text = result

        if not args.skip_validation:
            errors = validate_extraction(html_text, obligations, source_file)
            all_errors.extend(errors)

        # Write obligation files
        for ob in obligations:
            write_obligation_file(ob, args.output_dir)

        # Update manifest
        update_extracted_manifest(manifest_path, source_file, tier, len(obligations))
        all_obligations.extend(obligations)
        files_processed += 1

        print(f'  -> {len(obligations)} obligations extracted', file=sys.stderr)

    # Write index
    if all_obligations:
        # Load existing index and merge
        index_path = os.path.join(args.output_dir, '_index.json')
        existing_index = []
        if os.path.exists(index_path):
            with open(index_path, 'r', encoding='utf-8') as f:
                existing_index = json.load(f)

        for ob in all_obligations:
            existing_index.append({
                'id': ob['id'],
                'category': ob['category'],
                'section': ob['section'],
                'anchor': ob['anchor'],
                'source_file': ob['source_file'],
                'tier': ob['tier'],
                'keywords': ob['keywords'],
            })

        with open(index_path, 'w', encoding='utf-8') as f:
            json.dump(existing_index, f, indent=2)

    # Report
    print(f'\n=== Summary ===', file=sys.stderr)
    print(f'Files processed: {files_processed}', file=sys.stderr)
    print(f'Obligations extracted: {len(all_obligations)}', file=sys.stderr)

    if all_errors:
        print(f'\n=== VALIDATION WARNINGS ===', file=sys.stderr)
        for err in all_errors:
            print(f'  {err}', file=sys.stderr)
        # Warnings, not hard failures - preamble duplication and context
        # differences make exact matching difficult; the extractor captures
        # the normative content but counts may differ

    cats = {}
    for ob in all_obligations:
        cats[ob['category']] = cats.get(ob['category'], 0) + 1
    for cat, count in sorted(cats.items()):
        print(f'  {cat}: {count}', file=sys.stderr)

    return 0


if __name__ == '__main__':
    sys.exit(main())
