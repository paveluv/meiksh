import json
import re
import os
from html.parser import HTMLParser

class PosixParser(HTMLParser):
    def __init__(self):
        super().__init__()
        self.in_relevant_tag = False
        self.current_text = []
        self.requirements = []
        self.current_section = "Unknown"
        self.req_counter = 1
        self.relevant_tags = {'p', 'li', 'dd'}
        self.tag_stack = []

    def handle_starttag(self, tag, attrs):
        self.tag_stack.append(tag)
        if tag in ['h2', 'h3', 'h4', 'h5', 'h6']:
            self.in_relevant_tag = False
            self.current_text = []
            
    def handle_endtag(self, tag):
        if self.tag_stack:
            self.tag_stack.pop()
        
        if tag in ['h2', 'h3', 'h4', 'h5', 'h6']:
            section_title = "".join(self.current_text).strip()
            if section_title:
                # E.g. "2.2.1 Escape Character (Backslash)"
                match = re.match(r'^([\d\.]+)\s+(.*)', section_title)
                if match:
                    self.current_section = match.group(1)
                else:
                    self.current_section = section_title
            self.current_text = []
            
        elif tag in self.relevant_tags:
            text = "".join(self.current_text).strip()
            # Normalize whitespace
            text = re.sub(r'\s+', ' ', text)
            if text:
                # Split into sentences roughly
                sentences = re.split(r'(?<=[.!?])\s+', text)
                for sentence in sentences:
                    if re.search(r'\bshall\b', sentence, re.IGNORECASE):
                        req_id = f"SHALL-{self.current_section.replace('.', '-')}-{self.req_counter:03d}"
                        self.requirements.append({
                            "id": req_id,
                            "section": self.current_section,
                            "text": sentence
                        })
                        self.req_counter += 1
            self.current_text = []

    def handle_data(self, data):
        if self.tag_stack:
            parent = self.tag_stack[-1]
            if parent in self.relevant_tags or parent in ['h2', 'h3', 'h4', 'h5', 'h6']:
                self.current_text.append(data)
            elif parent in ['a', 'b', 'i', 'tt', 'code', 'em', 'strong']:
                # If we are inside a formatting tag that's inside a relevant tag
                if any(t in self.relevant_tags or t in ['h2', 'h3', 'h4', 'h5', 'h6'] for t in self.tag_stack):
                    self.current_text.append(data)

def process_file(filepath):
    parser = PosixParser()
    with open(filepath, 'r', encoding='utf-8') as f:
        content = f.read()
    parser.feed(content)
    return parser.requirements

def main():
    files = [
        "docs/posix/utilities/V3_chap02.html",
        "docs/posix/utilities/sh.html"
    ]
    all_reqs = []
    for f in files:
        if os.path.exists(f):
            all_reqs.extend(process_file(f))
    
    with open("tests/matrix/requirements.json", "w", encoding="utf-8") as out:
        json.dump(all_reqs, out, indent=2)
    print(f"Extracted {len(all_reqs)} requirements.")

if __name__ == "__main__":
    main()
