#!/usr/bin/env python3
"""
Audit tests/matrix requirements for hidden qualifiers and truncated extractions.

Detects two classes of problems that have caused past misinterpretations:
  1. Truncated requirement text that lost normative "may"/"unspecified" clauses
  2. Cross-file qualifiers (a "may" in V3_chap02 constraining a "shall" in a
     utility section) that are invisible when reading a single requirement

Usage:
    python3 scripts/audit-qualifiers.py [--repo-root DIR]

Writes tests/matrix/qualifier-audit.md
"""

import argparse
import json
import os
import re
import sys
from collections import defaultdict


QUALIFIER_KEYWORDS = re.compile(
    r'\b(may|unspecified|implementation-defined)\b', re.I
)

UTILITY_FILES = frozenset({
    'alias', 'bg', 'cd', 'command', 'echo', 'false', 'fc', 'fg',
    'getopts', 'hash', 'jobs', 'kill', 'printf', 'pwd', 'read',
    'test', 'true', 'type', 'ulimit', 'umask', 'unalias', 'wait',
})

SHELL_LANG_FILES = frozenset({'V3_chap02', 'sh'})


def load_requirements(path):
    with open(path, encoding='utf-8') as f:
        return json.load(f)


def collect_test_refs(test_dir):
    """Return {test_filename: set_of_requirement_ids} and global set."""
    per_file = {}
    all_refs = set()
    for fn in sorted(os.listdir(test_dir)):
        if not fn.endswith('.sh'):
            continue
        with open(os.path.join(test_dir, fn), encoding='utf-8') as f:
            content = f.read()
        ids = set(re.findall(r'REQUIREMENT:\s*(SHALL-\S+?):', content))
        per_file[fn] = ids
        all_refs.update(ids)
    return per_file, all_refs


# ---------------------------------------------------------------------------
# Phase 1: Truncated requirements
# ---------------------------------------------------------------------------

def detect_truncation(reqs):
    """Flag requirements whose text appears truncated."""
    findings = []

    for r in reqs:
        text = r['text']
        rid = r['id']
        reasons = []

        if re.search(r'shall be as follows\b', text, re.I):
            named_fields = re.findall(r'< (\w[\w -]*?) >', text)
            defined_fields = re.findall(
                r'< (\w[\w -]*?) >\s+(?:The |A |An |One )', text
            )
            if named_fields and len(defined_fields) < len(set(named_fields)):
                missing = set(named_fields) - set(defined_fields)
                reasons.append(
                    f'format defines {len(set(named_fields))} fields '
                    f'but only {len(defined_fields)} are described; '
                    f'missing: {", ".join(sorted(missing))}'
                )

        stripped = text.rstrip()
        if stripped.endswith(':'):
            reasons.append('text ends with a colon (likely truncated list)')
        if re.search(r'\b(following|as follows)\s*$', stripped, re.I):
            reasons.append('text ends with "following"/"as follows" with no list')

        if len(text) > 200 and not text.rstrip().endswith('.'):
            last_sentence = text.rstrip().split('.')[-1].strip()
            if len(last_sentence) > 80 and not re.search(r'[.!?;)\]"\']$', text.rstrip()):
                reasons.append(
                    'long text does not end with sentence-ending punctuation'
                )

        if reasons:
            findings.append((rid, r['section'], r['file'], reasons, text))

    return findings


# ---------------------------------------------------------------------------
# Phase 2: Cross-file qualifiers
# ---------------------------------------------------------------------------

def build_indices(reqs):
    by_id = {r['id']: r for r in reqs}
    by_file = defaultdict(list)
    by_section = defaultdict(list)
    for r in reqs:
        by_file[r['file']].append(r)
        by_section[(r['file'], r['section'])].append(r)
    return by_id, by_file, by_section


def find_cross_file_qualifiers(reqs, per_file_refs, all_refs):
    """For each utility tested, find V3_chap02/sh 'may' clauses mentioning it."""
    by_id, by_file, _ = build_indices(reqs)

    shell_lang_reqs = []
    for f in SHELL_LANG_FILES:
        shell_lang_reqs.extend(by_file.get(f, []))

    findings = []

    for test_fn, ref_ids in sorted(per_file_refs.items()):
        utility_names = set()
        for rid in ref_ids:
            r = by_id.get(rid)
            if r and r['file'] in UTILITY_FILES:
                utility_names.add(r['file'])

        if not utility_names:
            continue

        for uname in sorted(utility_names):
            pat = re.compile(r'\b' + re.escape(uname) + r'\b', re.I)
            for slr in shell_lang_reqs:
                if not pat.search(slr['text']):
                    continue
                if not QUALIFIER_KEYWORDS.search(slr['text']):
                    continue
                if slr['id'] in all_refs:
                    continue
                findings.append((test_fn, uname, slr))

    return findings


def find_same_file_qualifiers(reqs, per_file_refs, all_refs):
    """Find 'may'/'unspecified' siblings in the same POSIX file not cited by tests."""
    by_id, by_file, _ = build_indices(reqs)

    findings = []

    for test_fn, ref_ids in sorted(per_file_refs.items()):
        involved_files = set()
        for rid in ref_ids:
            r = by_id.get(rid)
            if r:
                involved_files.add(r['file'])

        for pfile in sorted(involved_files):
            siblings = by_file.get(pfile, [])
            for sib in siblings:
                if sib['id'] in all_refs:
                    continue
                if not QUALIFIER_KEYWORDS.search(sib['text']):
                    continue
                if not sib.get('testable', True):
                    continue
                findings.append((test_fn, pfile, sib))

    return findings


# ---------------------------------------------------------------------------
# Phase 3: Self-qualifying requirements
# ---------------------------------------------------------------------------

def find_self_qualifying(reqs, all_refs):
    """Find requirements containing both 'shall' and 'may'/'unspecified'."""
    findings = []
    for r in reqs:
        text_lower = r['text'].lower()
        if 'shall' not in text_lower:
            continue
        quals = QUALIFIER_KEYWORDS.findall(r['text'])
        if not quals:
            continue
        citing_tests = []
        if r['id'] in all_refs:
            citing_tests.append(r['id'])
        findings.append((r, sorted(set(q.lower() for q in quals)), r['id'] in all_refs))
    return findings


# ---------------------------------------------------------------------------
# Report generation
# ---------------------------------------------------------------------------

def write_report(path, truncation, cross_file, same_file, self_qual):
    lines = []
    w = lines.append

    w('# POSIX Qualifier Audit Report')
    w('')
    w('Generated by `scripts/audit-qualifiers.py`.  ')
    w('Review each flagged item against the POSIX HTML source to confirm or dismiss.')
    w('')

    # --- Phase 1 ---
    w('## 1. Potentially Truncated Requirements')
    w('')
    if not truncation:
        w('No truncation detected.')
    else:
        w(f'{len(truncation)} requirement(s) flagged.')
        w('')
        for rid, section, pfile, reasons, text in truncation:
            w(f'### `{rid}` (section {section}, file: {pfile})')
            w('')
            for reason in reasons:
                w(f'- {reason}')
            w('')
            preview = text[:300].replace('\n', ' ')
            if len(text) > 300:
                preview += '...'
            w(f'> {preview}')
            w('')

    # --- Phase 2a: cross-file ---
    w('## 2. Unaddressed Cross-File Qualifiers')
    w('')
    w('These are "may"/"unspecified"/"implementation-defined" requirements in the shell')
    w('language chapter (V3_chap02, sh) that mention a utility being tested but are not')
    w('cited by any test file.')
    w('')
    if not cross_file:
        w('None found.')
    else:
        grouped = defaultdict(list)
        for test_fn, uname, slr in cross_file:
            grouped[(test_fn, uname)].append(slr)
        for (test_fn, uname), slrs in sorted(grouped.items()):
            w(f'### {test_fn} (utility: {uname})')
            w('')
            seen = set()
            for slr in slrs:
                if slr['id'] in seen:
                    continue
                seen.add(slr['id'])
                preview = slr['text'][:250].replace('\n', ' ')
                if len(slr['text']) > 250:
                    preview += '...'
                w(f'- **`{slr["id"]}`** [{slr["section"]}]: {preview}')
                w('')
    w('')

    # --- Phase 2b: same-file ---
    w('## 3. Same-File Uncited Qualifiers')
    w('')
    w('These are "may"/"unspecified"/"implementation-defined" requirements in the same')
    w('POSIX source file as tested SHALLs, but not cited by any test.')
    w('')
    if not same_file:
        w('None found.')
    else:
        grouped = defaultdict(list)
        seen_ids = set()
        for test_fn, pfile, sib in same_file:
            if sib['id'] not in seen_ids:
                grouped[pfile].append(sib)
                seen_ids.add(sib['id'])
        for pfile, sibs in sorted(grouped.items()):
            w(f'### File: {pfile} ({len(sibs)} uncited qualifier(s))')
            w('')
            for sib in sibs:
                preview = sib['text'][:250].replace('\n', ' ')
                if len(sib['text']) > 250:
                    preview += '...'
                w(f'- **`{sib["id"]}`** [{sib["section"]}]: {preview}')
                w('')
    w('')

    # --- Phase 3 ---
    w('## 4. Self-Qualifying Requirements')
    w('')
    w('Requirements containing both "shall" and "may"/"unspecified"/"implementation-defined".')
    w('These need manual review to confirm the test accounts for the qualifier.')
    w('')
    cited = [(r, quals) for r, quals, is_cited in self_qual if is_cited]
    uncited = [(r, quals) for r, quals, is_cited in self_qual if not is_cited]

    if cited:
        w(f'### Cited by tests ({len(cited)})')
        w('')
        for r, quals in cited:
            qual_str = ', '.join(quals)
            preview = r['text'][:200].replace('\n', ' ')
            if len(r['text']) > 200:
                preview += '...'
            w(f'- **`{r["id"]}`** [{r["section"]}] qualifiers: {qual_str}')
            w(f'  > {preview}')
            w('')

    if uncited:
        w(f'### Not cited by any test ({len(uncited)})')
        w('')
        for r, quals in uncited:
            qual_str = ', '.join(quals)
            preview = r['text'][:200].replace('\n', ' ')
            if len(r['text']) > 200:
                preview += '...'
            w(f'- **`{r["id"]}`** [{r["section"]}] qualifiers: {qual_str}')
            w(f'  > {preview}')
            w('')

    with open(path, 'w', encoding='utf-8') as f:
        f.write('\n'.join(lines) + '\n')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        '--repo-root', default=None,
        help='Repository root (auto-detected if omitted)',
    )
    args = parser.parse_args()

    if args.repo_root:
        root = args.repo_root
    else:
        root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

    req_path = os.path.join(root, 'tests', 'matrix', 'requirements.json')
    test_dir = os.path.join(root, 'tests', 'matrix', 'tests')
    out_path = os.path.join(root, 'tests', 'matrix', 'qualifier-audit.md')

    if not os.path.isfile(req_path):
        print(f'ERROR: {req_path} not found', file=sys.stderr)
        return 1
    if not os.path.isdir(test_dir):
        print(f'ERROR: {test_dir} not found', file=sys.stderr)
        return 1

    reqs = load_requirements(req_path)
    per_file_refs, all_refs = collect_test_refs(test_dir)

    print(f'Loaded {len(reqs)} requirements, {len(per_file_refs)} test files, '
          f'{len(all_refs)} unique requirement references', file=sys.stderr)

    print('Phase 1: detecting truncated requirements...', file=sys.stderr)
    truncation = detect_truncation(reqs)
    print(f'  -> {len(truncation)} flagged', file=sys.stderr)

    print('Phase 2a: finding cross-file qualifiers...', file=sys.stderr)
    cross_file = find_cross_file_qualifiers(reqs, per_file_refs, all_refs)
    print(f'  -> {len(cross_file)} flagged', file=sys.stderr)

    print('Phase 2b: finding same-file uncited qualifiers...', file=sys.stderr)
    same_file = find_same_file_qualifiers(reqs, per_file_refs, all_refs)
    print(f'  -> {len(same_file)} flagged', file=sys.stderr)

    print('Phase 3: finding self-qualifying requirements...', file=sys.stderr)
    self_qual = find_self_qualifying(reqs, all_refs)
    print(f'  -> {len(self_qual)} flagged', file=sys.stderr)

    write_report(out_path, truncation, cross_file, same_file, self_qual)
    print(f'\nReport written to {out_path}', file=sys.stderr)
    return 0


if __name__ == '__main__':
    sys.exit(main())
