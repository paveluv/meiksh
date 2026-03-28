import json
import re
import os
from bs4 import BeautifulSoup

def normalize(text):
    text = re.sub(r'<[^>]+>', '', text)
    text = re.sub(r'\s+', '', text)
    return text.lower().strip()

def process_file(filepath):
    requirements = []
    with open(filepath, 'r', encoding='utf-8') as f:
        soup = BeautifulSoup(f, "html.parser")

    for element in soup(["script", "style", "title", "head"]):
        element.decompose()

    current_section = "Unknown"
    base_name = os.path.basename(filepath).replace('.html', '')

    block_tags = ['p', 'li', 'dd', 'td', 'th', 'div', 'pre', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'blockquote', 'tr', 'table', 'dl', 'dt', 'ul', 'ol']
    
    # Insert period at the end of block tags to force sentence splitting
    for tag in soup.find_all(block_tags):
        tag.append(" . ")

    text = soup.get_text(separator=" ", strip=True)
    text = text.replace('\xa0', ' ')
    text = re.sub(r'\s+', ' ', text)
    
    sentences = re.split(r'(?<=[.!?])\s+', text)
    
    seen_texts = set()
    
    for sentence in sentences:
        if re.search(r'\bshall\b', sentence, re.IGNORECASE):
            # Clean up dangling periods we added
            sentence = re.sub(r'\s+\.\s*$', '.', sentence)
            sentence = sentence.strip()
            
            # Avoid duplicates within same file
            nt = normalize(sentence)
            if nt in seen_texts:
                continue
            seen_texts.add(nt)
            
            # We don't have accurate section numbers anymore, but we can use base_name.
            # For V3_chap02 we could parse them out, but it's simpler to just use base_name.
            requirements.append({
                "section": base_name.upper(),
                "text": sentence,
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
        "docs/posix/utilities/fc.html",
        "docs/posix/utilities/fg.html",
        "docs/posix/utilities/getopts.html",
        "docs/posix/utilities/hash.html",
        "docs/posix/utilities/jobs.html",
        "docs/posix/utilities/kill.html",
        "docs/posix/utilities/read.html",
        "docs/posix/utilities/type.html",
        "docs/posix/utilities/ulimit.html",
        "docs/posix/utilities/umask.html",
        "docs/posix/utilities/unalias.html",
        "docs/posix/utilities/wait.html"
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
            # preserve original section from old req
            section = o_req['section']
        else:
            for o_ntext, o_req_list in old_norms.items():
                if not o_req_list:
                    continue
                if o_ntext in ntext or ntext in o_ntext:
                    o_req = o_req_list.pop(0)
                    matched_id = o_req['id']
                    section = o_req['section']
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
            section_clean = re.sub(r'[^A-Za-z0-9-]', '', nr['section'].replace(' ', '-'))
            new_id = f"SHALL-{section_clean}-{new_counter}"
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
