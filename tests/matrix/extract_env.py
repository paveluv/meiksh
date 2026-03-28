import json
import re
from bs4 import BeautifulSoup

def process_env():
    filepath = "docs/posix/basedefs/V1_chap08.html"
    requirements = []
    with open(filepath, 'r', encoding='utf-8') as f:
        soup = BeautifulSoup(f, "html.parser")

    for element in soup(["script", "style", "title", "head"]):
        element.decompose()

    # Don't add dot to dt or inline elements, only actual blocks
    block_tags = ['p', 'li', 'div', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'blockquote']
    for tag in soup.find_all(block_tags):
        tag.append(" . ")

    text = soup.get_text(separator=" ", strip=True)
    text = text.replace('\xa0', ' ')
    text = re.sub(r'\s+', ' ', text)
    
    sentences = re.split(r'(?<=[.!?])\s+', text)
    seen_texts = set()
    
    relevant_vars = {"ENV", "HOME", "IFS", "LANG", "LC_ALL", "LC_COLLATE", 
                     "LC_CTYPE", "LC_MESSAGES", "LINENO", "NLSPATH", "PATH", 
                     "PPID", "PS1", "PS2", "PS4", "PWD", "OPTIND", "OPTARG"}
                     
    for sentence in sentences:
        if re.search(r'\bshall\b', sentence, re.IGNORECASE):
            if any(re.search(r'\b' + v + r'\b', sentence) for v in relevant_vars):
                sentence = re.sub(r'\s+\.\s*$', '.', sentence).strip()
                
                nt = re.sub(r'<[^>]+>', '', sentence)
                nt = re.sub(r'\s+', '', nt).lower().strip()
                
                if nt in seen_texts: continue
                seen_texts.add(nt)
                
                requirements.append({
                    "section": "8. Environment Variables",
                    "text": sentence,
                    "file": "V1_chap08"
                })
            
    return requirements

def main():
    with open("tests/matrix/requirements.json", "r") as f:
        reqs = json.load(f)
        
    reqs = [r for r in reqs if r.get('file') != 'V1_chap08']
    
    new_reqs = process_env()
    print(f"Adding {len(new_reqs)} new requirements from XBD Chapter 8 Environment Variables")
    
    new_counter = 3000
    for nr in new_reqs:
        nr['id'] = f"SHALL-XBD-8-{new_counter}"
        new_counter += 1
        reqs.append(nr)
        
    with open("tests/matrix/requirements.json", "w") as f:
        json.dump(reqs, f, indent=2)

if __name__ == "__main__":
    main()
