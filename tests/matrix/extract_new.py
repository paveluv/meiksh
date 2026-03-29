import json
import re
import os

from extract import normalize, process_file


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
