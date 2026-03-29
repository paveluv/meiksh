import json
import re
import glob

def gather_covered_ids():
    covered_ids = set()
    test_files = glob.glob('tests/matrix/tests/*.sh')
    for tf in test_files:
        with open(tf, 'r', encoding='utf-8') as file:
            content = file.read()
            matches = re.findall(r'# REQUIREMENT:\s*(SHALL-[-0-9a-zA-Z.]+):', content)
            covered_ids.update(matches)
    return covered_ids

def main():
    with open('tests/matrix/requirements.json', 'r', encoding='utf-8') as f:
        requirements = json.load(f)

    covered_ids = gather_covered_ids()

    total_reqs = len(requirements)
    testable_reqs = [r for r in requirements if r.get('testable', True)]
    untestable_reqs = [r for r in requirements if not r.get('testable', True)]
    testable_count = len(testable_reqs)
    covered_count = len(covered_ids)
    testable_covered = len([r for r in testable_reqs if r['id'] in covered_ids])
    testable_pct = (testable_covered / testable_count) * 100 if testable_count > 0 else 0
    overall_pct = (covered_count / total_reqs) * 100 if total_reqs > 0 else 0

    print("=== POSIX Shell Compliance Matrix ===")
    print(f"Total Normative Requirements: {total_reqs}")
    print(f"  Testable:   {testable_count}")
    print(f"  Untestable: {len(untestable_reqs)}")
    print()
    print(f"Requirements Covered by Tests: {covered_count}")
    print(f"  Overall Coverage:  {covered_count}/{total_reqs} ({overall_pct:.1f}%)")
    print(f"  Testable Coverage: {testable_covered}/{testable_count} ({testable_pct:.1f}%)")

    reasons = {}
    for r in untestable_reqs:
        reason = r.get('untestable_reason', 'unknown')
        reasons[reason] = reasons.get(reason, 0) + 1
    if reasons:
        print(f"\nUntestable Breakdown:")
        for reason, count in sorted(reasons.items(), key=lambda x: -x[1]):
            print(f"  {reason:30s}: {count:4d}")

    print(f"\nCovered Requirements:")
    for rid in sorted(covered_ids):
        text = next((r['text'] for r in requirements if r['id'] == rid), "Unknown requirement text")
        print(f"  [X] {rid}: {text[:80]}...")

    remaining = testable_count - testable_covered
    print(f"\nNext Steps for 100% Testable Coverage:")
    print(f"  There are {remaining} testable requirements remaining to cover.")
    print("  Run 'python3 tests/matrix/report.py --missing' to list them.")

if __name__ == "__main__":
    import sys
    if '--missing' in sys.argv:
        with open('tests/matrix/requirements.json', 'r', encoding='utf-8') as f:
            requirements = json.load(f)
        covered_ids = gather_covered_ids()

        testable_only = '--testable' in sys.argv
        print("\nMissing Requirements:" + (" (testable only)" if testable_only else ""))
        for r in requirements:
            if r['id'] not in covered_ids:
                if testable_only and not r.get('testable', True):
                    continue
                marker = " [untestable]" if not r.get('testable', True) else ""
                print(f"  [ ] {r['id']}{marker}: {r['text'][:100]}...")
    elif '--untestable' in sys.argv:
        with open('tests/matrix/requirements.json', 'r', encoding='utf-8') as f:
            requirements = json.load(f)
        print("\nUntestable Requirements:")
        for r in requirements:
            if not r.get('testable', True):
                reason = r.get('untestable_reason', 'unknown')
                print(f"  {r['id']:45s} [{reason}]: {r['text'][:80]}...")
    else:
        main()
