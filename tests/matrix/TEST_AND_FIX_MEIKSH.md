# TEST_AND_FIX_MEIKSH procedure

> **Authority:** `docs/IMPLEMENTATION_POLICY.md` is the canonical source for
> project-wide rules.  If anything in this procedure conflicts with the
> implementation policy, the policy takes priority.

When the user asks to run this procedure on a test suite file (e.g.
`tests/matrix/tests/2_9_2_pipelines.md`), execute these phases **in order**.

---

## Phase 1 — Run the test suite against meiksh

```bash
cargo build --quiet
cargo run --quiet --bin expect_pty -- \
  --shell "$(pwd)/target/debug/meiksh" \
  tests/matrix/tests/<FILE>.md
```

Collect every `FAIL` line. If all tests pass, skip to Phase 4.

To re-run a single test for debugging:

```bash
cargo run --quiet --bin expect_pty -- \
  --shell "$(pwd)/target/debug/meiksh" \
  --test "test name" \
  tests/matrix/tests/<FILE>.md
```

---

## Phase 2 — Triage each failure

For **every** failure, determine root cause:

### 2a. Read the POSIX standard text

The `.md` file itself contains the verbatim standard text in `## Section`
blocks. Read the normative "shall" statements carefully. If a link needs
following, paths are repo-relative (e.g. `docs/posix/md/utilities/V3_chap02.md`).

### 2b. Classify

| Verdict | Meaning | Action |
|---------|---------|--------|
| **Test bug** | The test overasserts, underasserts, or contradicts POSIX | Fix the test (Phase 3a) |
| **meiksh bug** | meiksh violates a "shall" statement | Fix meiksh source (Phase 3b) |

Check `tests/matrix/bash_compliance.md` for known bash deviations — a test
that fails against bash is fine if POSIX requires it.

---

## Test sandbox

Every test runs inside a unique, initially empty temporary directory created by
`expect_pty`. The directory is automatically removed after the test finishes
(whether it passes, fails, or times out). Tests can therefore:

- Assume the current working directory is empty at the start.
- Create files and directories freely — no cleanup is needed.
- Rely on glob patterns like `*` matching only files the test itself created.

`HOME` and `TMPDIR` point to the parent of the sandbox directory. Coverage
profile files (`LLVM_PROFILE_FILE`) are redirected there as well, so they never
appear inside the sandbox.

---

## Phase 3a — Fix test bugs

Follow the rules in `tests/matrix/MD_TEST_SUITES.md`:

- Assert only what POSIX explicitly specifies ("shall" statements).
- Don't overassert (e.g. exact wording of diagnostics — use `".+"` for
  required-but-unspecified diagnostic text).
- Don't underassert (e.g. `""` for stderr when a diagnostic is required).
- Every test needs a brief plain-English description between the `#### Test:`
  heading and the code block.
- Test names must match exactly between the heading and `begin test`/`end test`.
- Assertions must appear in order: `stdout`, `stderr`, `exit_code` (all three
  required).

After edits, verify parsing:

```bash
cargo run --quiet --bin expect_pty -- \
  --shell /usr/bin/bash --parse-only \
  tests/matrix/tests/<FILE>.md
```

---

## Phase 3b — Fix meiksh source bugs

For each meiksh bug, follow a **test-first** workflow:

1. **Write a failing test first.** Add a unit test (in the relevant
   `src/..` mod tests block) or an integration test (in
   `tests/integration/`) that reproduces the exact bug. See
   `docs/TEST_WRITING_GUIDE.md` for guidance on choosing the right test
   level and for available helpers. The test must **fail** against the
   current code — run it to confirm.
2. **Fix the bug** in the meiksh source so the new test passes.
3. **Run `cargo test`** to verify all existing tests still pass alongside
   the new one.

When writing the fix, apply these constraints strictly:

- **Zero-copy philosophy**: minimize allocations and copies. Prefer
  `&[u8]` / `Cow<'_, [u8]>` over `Vec<u8>`. Prefer slicing over cloning.
  Prefer `Box<[u8]>` over `Vec<u8>` and `Box<[T]>` over `Vec<T>` for
  frozen (immutable after creation) owned data — they drop the capacity
  word and signal the value won't be mutated. (`String` and `str`-based
  types are banned in production code — see `docs/IMPLEMENTATION_POLICY.md`.)
- **No unnecessary allocations**: don't allocate a `Vec` when a fixed-size
  local or iterator suffices.
- **Clean, minimal diffs**: change only what is necessary. Don't refactor
  surrounding code unless the fix requires it.
- **No narrating comments**: don't add comments that just describe what the
  code does. Only comment non-obvious intent or trade-offs.

---

## Phase 4 — Format and coverage

### 4a. Format

```bash
cargo fmt
```

### 4b. Run coverage

```bash
bash scripts/coverage.sh
```

The final line prints production-only coverage. The target is **100.00%**.

### 4c. If coverage < 100%

Find missed production lines:

```bash
python3 - "$(pwd)" target/coverage/prod-lcov.info target/coverage/lcov.info <<'PY'
import pathlib, sys
repo_root = pathlib.Path(sys.argv[1])
prod_lcov = pathlib.Path(sys.argv[2])
test_lcov = pathlib.Path(sys.argv[3])
def parse_lcov_lines(lcov_path):
    result = {}
    current = None
    for raw_line in lcov_path.read_text().splitlines():
        if raw_line.startswith("SF:"):
            p = pathlib.Path(raw_line[3:])
            if str(p).startswith(str(repo_root / "src")) and p.exists():
                current = p
                result.setdefault(current, {})
            else:
                current = None
            continue
        if not raw_line.startswith("DA:") or current is None:
            continue
        line_no, count = raw_line[3:].split(",")[:2]
        result[current][int(line_no)] = max(result[current].get(int(line_no), 0), int(count))
    return result
prod = parse_lcov_lines(prod_lcov)
test = parse_lcov_lines(test_lcov)
for path, lines in sorted(prod.items(), key=lambda x: str(x[0])):
    t = test.get(path, {})
    for lno in sorted(lines):
        if t.get(lno, 0) == 0:
            print(f"{path.relative_to(repo_root)}:{lno}")
PY
```

For each missed line, read the source, understand the code path, and add a
test that exercises it — either a unit test (in the relevant `src/` module) or
an integration test (in `tests/integration/`).  Use good judgement: prefer
whichever level gives the shortest, most readable test.  See
`docs/TEST_WRITING_GUIDE.md` for guidance on choosing and for the available
helpers (`test_shell()`, `run_trace`, `assert_no_syscalls`, `meiksh()`,
`run_meiksh_with_stdin()`, etc.).

Re-run `bash scripts/coverage.sh` and repeat until 100%.

---

## Phase 5 — Final verification

Re-run the original test suite to confirm everything still passes:

```bash
cargo run --quiet --bin expect_pty -- \
  --shell "$(pwd)/target/debug/meiksh" \
  tests/matrix/tests/<FILE>.md
```

Report a summary: how many tests failed initially, how many were test bugs
vs meiksh bugs, what was fixed, and final coverage.
