import json
import re
import os
from bs4 import BeautifulSoup


def normalize(text):
    text = re.sub(r'<[^>]+>', '', text)
    text = re.sub(r'\s+', '', text)
    return text.lower().strip()


def _detect_chapter_file(filepath):
    base = os.path.basename(filepath)
    return bool(re.match(r'V\d+_chap\d+', base))


def _get_visible_chapter(soup):
    h2 = soup.find('h2')
    if h2:
        text = h2.get_text(strip=True)
        m = re.match(r'(\d+)\.', text)
        if m:
            return int(m.group(1))
    return 0


def _resolve_section(tag_id, heading_text, visible_chapter, is_chapter_file):
    """Convert a tag_ anchor to a section string."""
    if is_chapter_file:
        parts = tag_id.replace('tag_', '').split('_')
        if len(parts) >= 2:
            sub_parts = parts[1:]
            section_nums = '.'.join(str(int(p)) for p in sub_parts)
            return f"{visible_chapter}.{section_nums}"
        return str(visible_chapter)
    # Utility page: use heading text, hyphenated
    label = re.sub(r'^\d+(\.\d+)*\s*', '', heading_text).strip()
    label = re.sub(r'[()]', '', label)
    label = re.sub(r'\s+', '-', label)
    label = re.sub(r'-+', '-', label).strip('-')
    return label


def process_file(filepath):
    requirements = []
    with open(filepath, 'r', encoding='utf-8') as f:
        soup = BeautifulSoup(f, "html.parser")

    for element in soup(["script", "style", "title", "head"]):
        element.decompose()

    base_name = os.path.basename(filepath).replace('.html', '')
    is_chapter_file = _detect_chapter_file(filepath)
    visible_chapter = _get_visible_chapter(soup) if is_chapter_file else 0

    # Build a section map: for each heading with a tag_ anchor,
    # record (element, section_string)
    section_entries = []
    for h in soup.find_all(['h2', 'h3', 'h4', 'h5', 'h6']):
        a = h.find('a', attrs={'name': re.compile(r'^tag_')})
        if not a:
            continue
        tag_id = a['name']
        heading_text = h.get_text(strip=True)
        section = _resolve_section(tag_id, heading_text, visible_chapter,
                                   is_chapter_file)
        section_entries.append((h, section))

    # Inject section markers into the DOM before flattening.
    # For each heading in section_entries, insert a sentinel text node
    # right after it so that the flattened text stream carries section info.
    SECTION_MARKER = "\x01SECT:"
    for h, section in section_entries:
        marker = soup.new_string(f" {SECTION_MARKER}{section}\x01 ")
        h.insert_after(marker)

    block_tags = ['p', 'li', 'dd', 'td', 'th', 'div', 'pre',
                  'h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'blockquote']
    for tag in soup.find_all(block_tags):
        tag.append(" \u2400 ")

    text = soup.get_text(separator=" ", strip=True)
    text = text.replace('\xa0', ' ')
    text = re.sub(r'\s+', ' ', text)

    blocks = [b.strip() for b in text.split(' \u2400 ') if b.strip()]

    merged_blocks = []
    i = 0
    while i < len(blocks):
        curr = blocks[i]
        if curr.endswith(':') or curr.endswith(','):
            merged = [curr]
            j = i + 1
            while j < len(blocks):
                last_block = merged[-1].replace('\u2400', '').strip()
                next_block = blocks[j].replace('\u2400', '').strip()

                if re.search(r'[.?]$', next_block) and not next_block.endswith('!'):
                    merged.append(blocks[j])
                    j += 1
                    break

                if j > i:
                    if (not re.search(r'[.!?:,;]$', last_block) and
                            re.match(r'^[A-Z]', next_block)):
                        if re.search(r'\bshall\b', next_block, re.IGNORECASE):
                            break

                merged.append(blocks[j])

                if j - i > 60:
                    j += 1
                    break
                j += 1
            curr = " ".join(merged)
            i = j
        else:
            i += 1
        merged_blocks.append(curr)

    # Extract sentences from merged blocks, tracking current section
    # via the injected markers.
    seen_texts = set()
    current_section = base_name.upper()

    for block in merged_blocks:
        block = block.replace(" \u2400 ", " ").replace("\u2400", "").strip()

        # Extract and consume section markers from this block
        while SECTION_MARKER in block:
            start = block.index(SECTION_MARKER)
            end = block.index('\x01', start + len(SECTION_MARKER))
            current_section = block[start + len(SECTION_MARKER):end]
            block = block[:start] + block[end + 1:]
        block = block.strip()
        if not block:
            continue

        sentences = re.split(
            r'(?<=\.)\s+(?=[A-Z])|(?<=\?)\s+(?=[A-Z])', block)
        for sentence in sentences:
            # Clean any remaining markers from sentence fragments
            clean = re.sub(r'\x01SECT:[^\x01]*\x01', '', sentence).strip()
            if not clean:
                continue
            if re.search(r'\bshall\b', clean, re.IGNORECASE):
                nt = normalize(clean)
                if nt in seen_texts:
                    continue
                seen_texts.add(nt)

                requirements.append({
                    "section": current_section,
                    "text": clean,
                    "file": base_name
                })

    return requirements


def main():
    with open("tests/matrix/requirements.json", "r") as f:
        old_reqs = json.load(f)

    old_norms = {}
    for r in old_reqs:
        ntext = normalize(r['text'])
        if ntext not in old_norms:
            old_norms[ntext] = []
        old_norms[ntext].append(r)

    files = [
        "docs/posix/utilities/V3_chap02.html",
        "docs/posix/utilities/sh.html",
        "docs/posix/utilities/alias.html",
        "docs/posix/utilities/bg.html",
        "docs/posix/utilities/cd.html",
        "docs/posix/utilities/command.html",
        "docs/posix/utilities/echo.html",
        "docs/posix/utilities/env.html",
        "docs/posix/utilities/false.html",
        "docs/posix/utilities/fc.html",
        "docs/posix/utilities/fg.html",
        "docs/posix/utilities/getopts.html",
        "docs/posix/utilities/hash.html",
        "docs/posix/utilities/jobs.html",
        "docs/posix/utilities/kill.html",
        "docs/posix/utilities/newgrp.html",
        "docs/posix/utilities/printf.html",
        "docs/posix/utilities/pwd.html",
        "docs/posix/utilities/read.html",
        "docs/posix/utilities/stty.html",
        "docs/posix/utilities/test.html",
        "docs/posix/utilities/[.html",
        "docs/posix/utilities/true.html",
        "docs/posix/utilities/type.html",
        "docs/posix/utilities/ulimit.html",
        "docs/posix/utilities/umask.html",
        "docs/posix/utilities/unalias.html",
        "docs/posix/utilities/wait.html",
    ]

    new_reqs_raw = []
    for f in files:
        if os.path.exists(f):
            new_reqs_raw.extend(process_file(f))

    final_reqs = []
    new_counter = 1000
    matched_old_ids = set()

    for nr in new_reqs_raw:
        ntext = normalize(nr['text'])

        matched_id = None
        if ntext in old_norms and len(old_norms[ntext]) > 0:
            o_req = old_norms[ntext].pop(0)
            matched_id = o_req['id']
            section = nr['section']
        else:
            for o_ntext, o_req_list in old_norms.items():
                if not o_req_list:
                    continue
                if o_ntext in ntext or ntext in o_ntext:
                    o_req = o_req_list.pop(0)
                    matched_id = o_req['id']
                    section = nr['section']
                    break

        if matched_id:
            matched_old_ids.add(matched_id)
            final_reqs.append({
                "id": matched_id,
                "section": section,
                "text": nr['text'],
                "file": nr['file']
            })
        else:
            section_clean = re.sub(r'[^A-Za-z0-9.-]', '',
                                   nr['section'].replace(' ', '-'))
            section_for_id = section_clean.replace('.', '-')
            new_id = f"SHALL-{section_for_id}-{new_counter}"
            new_counter += 1
            final_reqs.append({
                "id": new_id,
                "section": nr['section'],
                "text": nr['text'],
                "file": nr['file']
            })

    unmatched = [r for r in old_reqs if r['id'] not in matched_old_ids]
    if unmatched:
        print(f"Adding {len(unmatched)} unmatched old requirements back.")
        for r in unmatched:
            r['file'] = r.get('file', 'Unknown')
            final_reqs.append(r)

    with open("tests/matrix/requirements.json", "w", encoding="utf-8") as out:
        json.dump(final_reqs, out, indent=2)
    print(f"Total old: {len(old_reqs)}")
    print(f"Total new raw: {len(new_reqs_raw)}")
    print(f"Total final: {len(final_reqs)}")


if __name__ == "__main__":
    main()
