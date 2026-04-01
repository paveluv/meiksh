# Testing Guide Enrichment Loop

This document describes the supervisor loop for bulk-enriching the
`testing_instructions` field across all requirements in `requirements.json`.

The procedure is designed to be restartable: pick up from wherever the
last run left off by finding the next unenriched requirement.

## Prerequisites

- `tests/matrix/testing_instructions_enrichment.md` defines the per-requirement
  enrichment procedure (steps 1–7).
- `tests/matrix/requirements.json` contains all requirements. A
  requirement is considered unenriched when its `testing_instructions` field is
  `null`.

## Finding the next requirement

Run this to get the first unenriched requirement:

```python
import json
with open('tests/matrix/requirements.json') as f:
    reqs = json.load(f)
for r in reqs:
    if r['testing_instructions'] is None:
        print(json.dumps(r, indent=2))
        break
```

To see overall progress:

```python
import json
with open('tests/matrix/requirements.json') as f:
    reqs = json.load(f)
unenriched = sum(1 for r in reqs if r['testing_instructions'] is None)
print(f'Total: {len(reqs)}, Enriched: {len(reqs) - unenriched}, Remaining: {unenriched}')
```

## The loop

Repeat the following cycle until no unenriched requirements remain:

### 1. Spawn a subagent for one requirement

Spawn exactly **one** subagent (Task tool, `subagent_type: generalPurpose`)
for the next unenriched requirement.

The subagent prompt must include:
- The full JSON of the requirement to enrich.
- An instruction to **read `tests/matrix/testing_instructions_enrichment.md`
  first** and follow its steps 1–7 exactly. Do not paraphrase the
  procedure in the prompt — the subagent must read it from the file.
- The validation command to run at the end (step 7 in the procedure).

The subagent is responsible for **all** deliverables before it finishes:
1. Writing the `testing_instructions` field in `requirements.json`.
2. Writing or updating **all** tests described by the testing guide in
   the appropriate `.epty` file(s).
3. Updating the `tests` array in `requirements.json` to reference every
   test.
4. Running the integrity check and confirming it passes.

If any of these are incomplete the enrichment is considered failed.

Do **not** spawn multiple subagents concurrently. One at a time ensures
focus and avoids file conflicts.

### 2. Wait for the subagent to finish

Monitor the subagent until it completes. If it gets stuck or is
aborted, check partial progress:

```bash
# Did it update the testing_instructions?
python3 -c "
import json
with open('tests/matrix/requirements.json') as f:
    reqs = json.load(f)
for r in reqs:
    if r['id'] == 'THE-ID':
        print('testing_instructions:', 'set' if r['testing_instructions'] else 'None')
        break
"

# Does integrity still pass?
cargo run --bin expect_pty -- --parse-only \
  --requirements tests/matrix/requirements.json \
  tests/matrix/tests/*.epty
```

If the subagent enriched the requirement and integrity passes, proceed
to step 3. If integrity fails, fix errors manually. If the subagent
made no changes, retry.

### 3. Commit

Commit all changes from the enrichment:

```bash
git add tests/matrix/requirements.json tests/matrix/tests/*.epty
git commit -m "enrich testing_instructions for <ID> (<brief description>)"
```

### 4. Go to step 1

Find the next unenriched requirement and repeat.

## Progress tracking

After each commit, optionally log progress:

```bash
python3 -c "
import json
with open('tests/matrix/requirements.json') as f:
    reqs = json.load(f)
done = sum(1 for r in reqs if r['testing_instructions'] is not None)
total = len(reqs)
print(f'Progress: {done}/{total} ({100*done/total:.1f}%)')
"
```

## Restarting

This loop is fully restartable. To resume from a new chat:

1. Read this file (`tests/matrix/testing_instructions_enrichment_loop.md`).
2. Check progress (see above).
3. Get the next unenriched requirement.
4. Continue the loop from step 1.

No state is kept outside of `requirements.json` and the `.epty` files —
everything needed to resume is in the repository.
