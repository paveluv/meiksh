import json
import re
import os
from bs4 import BeautifulSoup

def process_xbd():
    filepath = "docs/posix/basedefs/V1_chap09.html"
    requirements = []
    with open(filepath, 'r', encoding='utf-8') as f:
        soup = BeautifulSoup(f, "html.parser")

    for element in soup(["script", "style", "title", "head"]):
        element.decompose()

    # We only want section 9.3.5. 
    # Let's find the h4 tag with id="tag_09_03_05" or text "9.3.5 RE Bracket Expression"
    h4 = soup.find(lambda tag: tag.name == "h4" and "9.3.5" in tag.text)
    
    if not h4:
        print("Could not find 9.3.5")
        return []
        
    # collect all elements after h4 until next h4
    elements = []
    curr = h4.find_next_sibling()
    while curr and curr.name != "h4":
        elements.append(curr)
        curr = curr.find_next_sibling()
        
    block_tags = ['p', 'li', 'dd', 'td', 'th', 'div', 'pre', 'blockquote']
    
    text_chunks = []
    for tag in elements:
        if tag.name in block_tags or tag.find(block_tags):
            for t in tag.find_all(block_tags):
                t.append(" . ")
            text_chunks.append(tag.get_text(separator=" ", strip=True))
        else:
            text_chunks.append(tag.get_text(separator=" ", strip=True))
            
    text = " ".join(text_chunks)
    text = text.replace('\xa0', ' ')
    text = re.sub(r'\s+', ' ', text)
    
    sentences = re.split(r'(?<=[.!?])\s+', text)
    seen_texts = set()
    
    for sentence in sentences:
        if re.search(r'\bshall\b', sentence, re.IGNORECASE):
            sentence = re.sub(r'\s+\.\s*$', '.', sentence).strip()
            
            nt = re.sub(r'<[^>]+>', '', sentence)
            nt = re.sub(r'\s+', '', nt).lower().strip()
            
            if nt in seen_texts: continue
            seen_texts.add(nt)
            
            requirements.append({
                "section": "9.3.5 RE Bracket Expression",
                "text": sentence,
                "file": "V1_chap09"
            })
            
    return requirements

def main():
    new_reqs = process_xbd()
    with open("tests/matrix/requirements.json", "r") as f:
        reqs = json.load(f)
        
    print(f"Adding {len(new_reqs)} new requirements from XBD 9.3.5")
    
    # generate IDs
    new_counter = 2000
    for nr in new_reqs:
        nr['id'] = f"SHALL-XBD-9-3-5-{new_counter}"
        new_counter += 1
        reqs.append(nr)
        
    with open("tests/matrix/requirements.json", "w") as f:
        json.dump(reqs, f, indent=2)

if __name__ == "__main__":
    main()
