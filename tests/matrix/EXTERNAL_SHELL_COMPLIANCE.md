# External Shell POSIX Compliance Procedure

This procedure tests a given shell against the meiksh POSIX conformance test
suites and produces a compliance report documenting verified non-compliances.

## Inputs

The user provides:
- A shell invocation string (e.g. `"/usr/bin/bash --posix"`, `"/usr/bin/dash"`)

## Step 1: Run the test suites

Build the test runner and execute all `.md` test suites against the target shell
using `tests/matrix/run.sh`:

```bash
bash tests/matrix/run.sh \
  --shell "<shell invocation>" \
  --script-modes dash-c,tempfile,stdin
```

The `--script-modes dash-c,tempfile,stdin` flag runs each non-interactive test
three ways (as a `-c` argument, as a temp file, and via stdin) to catch
mode-specific differences. If a test fails in one mode but passes in others,
that is still a finding worth investigating.

To re-run a single failing test:

```bash
bash tests/matrix/run.sh \
  --shell "<shell invocation>" \
  --test "<test name>" \
  tests/matrix/tests/<file>.md
```

## Step 2: Triage every failure

For each failing test, determine the root cause. There are exactly three
possibilities:

### A) Real shell non-compliance

The shell's behavior contradicts the POSIX standard. To confirm:

1. Find the exact normative text in `docs/posix/md/` that specifies the
   expected behavior.
2. Quote it verbatim — do not paraphrase.
3. Construct a minimal standalone reproduction that uses only the shell
   binary and standard utilities (no test harness, no expect_pty, no `.md`
   files). The reproduction must work on any POSIX system with the shell
   installed.
4. Run the reproduction and record the expected vs. observed output.

If confirmed, add the finding to the compliance document (Step 3).

### B) Test bug

The test itself has a wrong expectation. To confirm:

1. Find the exact normative text in `docs/posix/md/` and verify the test's
   expectation contradicts it.
2. Fix the test to match the POSIX specification.

**Important:** Never bend a test to match a shell's behavior. The POSIX spec
is the authority. If a test expects behavior X, and the shell does Y, but the
spec says X — that is a shell non-compliance, not a test bug. Conversely, if
the spec says Y, fix the test regardless of what any particular shell does.

### C) Legitimate implementation variation

POSIX explicitly allows the behavior to vary (e.g. "implementation-defined",
"unspecified", or the feature is in an optional extension). Neither a shell
bug nor a test bug — skip it.

## Step 3: Write (or update) the compliance document

The compliance document lives at:

```
tests/matrix/<shell_name>_compliance.md
```

Where `<shell_name>` is the base name of the shell binary (e.g.
`bash_compliance.md`, `dash_compliance.md`, `zsh_compliance.md`).

### Document format

Use `tests/matrix/bash_compliance.md` as the reference. The structure is:

```markdown
# <Shell Name> POSIX Compliance Report (Verified Non-Compliances Only)

**Shell tested:** <exact version string from `<shell> --version` or similar>
**Standard:** POSIX.1-2024 (Issue 8)
**Date:** <YYYY-MM-DD>

This document intentionally lists **only verified non-compliances** that
can be reproduced directly with standard shell usage.

---

## <N>) <Short title>

**POSIX passage (exact quote)**
From `docs/posix/md/<path>`:

> "<verbatim quote from the standard>"

**Why this is non-compliant**
<1-3 sentences explaining the gap between spec and shell behavior.>

**Reproduction (portable shell commands)**

\```sh
<commands that demonstrate the bug using only the shell binary>
\```

Expected:
- <what POSIX requires>

Observed:
- <what the shell actually does>

---
```

### Rules for entries

- **Only real non-compliances.** Every entry must have a verbatim POSIX quote
  and a standalone reproduction. If you cannot independently reproduce it
  outside the test harness, do not include it.
- **Version-specific issues.** If a non-compliance is fixed in a newer version
  of the shell, note the affected versions (e.g. "Observed in bash 5.2;
  fixed in bash 5.3").
- **Portable reproduction.** The reproduction section must work on any POSIX
  system with the shell installed. Use absolute paths to the shell binary
  where practical. Do not reference expect_pty, `.md` test files, or any
  meiksh-specific tooling.
- **Cross-references welcome.** When another well-known shell handles the same
  case correctly, a brief cross-reference helps confirm the finding (e.g.
  "dash handles this correctly: `dash -c '...'`").
- **Sequential numbering.** Number entries sequentially. If entries are removed
  later, do not renumber — leave gaps.
- **Closing line.** End the document with:

  ```markdown
  This file is intentionally strict: only independently reproducible,
  standards-backed <shell> deviations are included.
  ```

## Step 4: Fix any test bugs found

If Step 2 identified test bugs (category B), fix them:

1. Edit the `.md` test file to correct the expectation.
2. Re-run the corrected test against both the external shell and meiksh to
   confirm it now passes (or fails only due to a genuine shell difference).
3. Run `cargo fmt` and `cargo test` to ensure nothing is broken.

## Step 5: Summary

After processing all failures, report:
- Total tests run
- Tests passed
- Real non-compliances found (added to compliance doc)
- Test bugs found and fixed
- Legitimate implementation variations (skipped)
