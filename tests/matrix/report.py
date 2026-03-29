import json
import os
import re
import glob

def main():
    with open('tests/matrix/requirements.json', 'r', encoding='utf-8') as f:
        requirements = json.load(f)

    # Find covered requirements
    covered_ids = set()
    test_files = glob.glob('tests/matrix/tests/*.sh')

    for tf in test_files:
        with open(tf, 'r', encoding='utf-8') as file:
            content = file.read()
            # Look for: # REQUIREMENT: SHALL-ID
            matches = re.findall(r'# REQUIREMENT:\s*(SHALL-[-0-9a-zA-Z.]+):', content)
            covered_ids.update(matches)

    total_reqs = len(requirements)
    covered_count = len(covered_ids)
    coverage_percent = (covered_count / total_reqs) * 100 if total_reqs > 0 else 0

    print("=== POSIX Shell Compliance Matrix ===")
    print(f"Total Normative Requirements Extracted: {total_reqs}")
    print(f"Requirements Covered by Tests: {covered_count}")
    print(f"Current Test Coverage: {coverage_percent:.2f}%\n")

    print("Covered Requirements:")
    for rid in sorted(covered_ids):
        # find the requirement text
        text = next((r['text'] for r in requirements if r['id'] == rid), "Unknown requirement text")
        print(f"  [X] {rid}: {text[:80]}...")

    print("\nNext Steps for 100% Coverage:")
    missing_count = total_reqs - covered_count
    print(f"  There are {missing_count} requirements remaining to implement.")
    print("  Run 'python3 tests/matrix/report.py --missing' to list them.")

if __name__ == "__main__":
    import sys
    if '--missing' in sys.argv:
        with open('tests/matrix/requirements.json', 'r', encoding='utf-8') as f:
            requirements = json.load(f)
        covered_ids = set()
        test_files = glob.glob('tests/matrix/tests/*.sh')
        for tf in test_files:
            with open(tf, 'r', encoding='utf-8') as file:
                content = file.read()
                matches = re.findall(r'# REQUIREMENT:\s*(SHALL-[-0-9a-zA-Z.]+):', content)
                covered_ids.update(matches)
                
        print("\nMissing Requirements:")
        for r in requirements:
            if r['id'] not in covered_ids:
                print(f"  [ ] {r['id']}: {r['text'][:100]}...")
    else:
        main()
