import json
import re
import os
from bs4 import BeautifulSoup

def normalize(text):
    text = re.sub(r'<[^>]+>', '', text)
    text = re.sub(r'\s+', '', text)
    return text.lower().strip()

def process_file(filepath, sections_to_extract=None):
    requirements = []
    with open(filepath, 'r', encoding='utf-8') as f:
        soup = BeautifulSoup(f, "html.parser")

    for element in soup(["script", "style", "title", "head"]):
        element.decompose()

    current_section = "Unknown"
    base_name = os.path.basename(filepath).replace('.html', '')

    block_tags = ['p', 'li', 'dd', 'td', 'th', 'div', 'pre', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'blockquote']
    
    for tag in soup.find_all(block_tags):
        tag.append(" . ")

    text = soup.get_text(separator=" ", strip=True)
    text = text.replace('\xa0', ' ')
    text = re.sub(r'\s+', ' ', text)
    
    sentences = re.split(r'(?<=[.!?])\s+', text)
    seen_texts = set()
    
    for sentence in sentences:
        if re.search(r'\bshall\b', sentence, re.IGNORECASE):
            sentence = re.sub(r'\s+\.\s*$', '.', sentence).strip()
            nt = normalize(sentence)
            if nt in seen_texts:
                continue
            seen_texts.add(nt)
            
            # Since we can't easily track current_section without a more complex tree walk, 
            # we'll just assign it to the base_name.
            requirements.append({
                "section": base_name.upper(),
                "text": sentence,
                "file": base_name
            })

    return requirements

def main():
    pass
