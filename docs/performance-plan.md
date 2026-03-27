# Performance Plan: Zero-Copy and Low-Allocation Strategies

This document captures a prioritized plan for reducing heap allocations, enabling
zero-copy data flow, and shrinking the binary footprint of meiksh.

## Current Allocation Profile

The same text is copied **at least 4-5 times** on its way from source to final
expanded argument:

```
source &str
  → Vec<char>             (tokenizer, 4x byte size)
  → String in Token       (owned copy of each word)
  → clone into AST Word   (parser clones token String)
  → Vec<char>             (expander, 4x again)
  → String in Segment     (per-char allocation in hot path)
  → String in Field       (field splitting materializes Vec<(char,bool)>)
  → Vec<String>           (final result)
```

There are **11 sites** that do `chars().collect::<Vec<char>>()` across the
tokenizer, expander, arithmetic parser, and pattern matcher. Each allocates 4x
the byte size of the input.

---

## Tier 1 — High Impact, Moderate Effort

### P1. Fix per-character String allocation in expand_raw

**File:** `src/expand.rs`, line 293

**Problem:** `ch.to_string()` allocates a new heap String for every literal
character in unquoted context. This is the hottest single allocation site.

**Fix:** Accumulate into a running `String` buffer and only flush to a segment
when quotedness changes or a non-text segment is needed:

```rust
// Instead of:
ch => {
    push_segment(&mut segments, ch.to_string(), false);
}

// Accumulate:
ch => {
    unquoted_buf.push(ch);
}
// Flush buffer before quote/expansion/backtick transitions
```

**Effort:** Small. Self-contained change within `expand_raw`.

**Impact:** Reduces O(n) allocations to O(1) amortized for runs of literal text.

---

### P2. Add release profile for binary size

**File:** `Cargo.toml`

**Problem:** No `[profile.release]` section exists. Cargo defaults leave unwind
tables, symbol tables, and dead `std` code in the binary.

**Fix:**

```toml
[profile.release]
lto = true
codegen-units = 1
strip = true
panic = "abort"
```

`panic = "abort"` alone saves ~100KB+ by eliminating unwind tables. `lto = true`
lets the linker remove unused `std` functions. `strip = true` removes symbol
names. `codegen-units = 1` enables better cross-module optimization.

Use `opt-level = 3` (default) for speed, or `opt-level = "z"` if binary size
matters more than throughput.

**Effort:** One-liner change.

**Impact:** Significant binary size reduction (~30-50% smaller).

---

### P3. Stop cloning the alias HashMap on every list item

**File:** `src/syntax.rs`, lines 200, 223

**Problem:** `self.parser.aliases = aliases.clone()` deep-copies the entire
`HashMap<String, String>` on every `ParseSession::next_item()` call. For a
script with 100 commands and 10 aliases, that is 100 full HashMap clones.

**Fix:** Change `Parser.aliases` from `HashMap<String, String>` (owned) to
`&'a HashMap<String, String>` (borrowed). The parser only reads aliases, never
mutates the map.

**Effort:** Small. Adds a lifetime to `Parser` and `ParseSession`, but the
caller already owns the aliases.

**Impact:** Eliminates O(commands x aliases) cloning.

---

### P4. Inline field splitting over segments directly

**File:** `src/expand.rs`, `split_fields_from_segments` (line 1110)

**Problem:** `flatten_segment_chars` (line 1200) builds a `Vec<(char, bool)>`
with one entry per character — a full copy of all text paired with per-character
quoting metadata. The segment list already encodes quoting at the segment level.

**Fix:** Iterate segments directly during field splitting. Walk segments in
order; for each `Text(content, quoted)`, iterate its chars and apply IFS
splitting logic with the known quotedness. No intermediate materialization.

**Effort:** Moderate. Rewrites the inner loop of `split_fields_from_segments`.

**Impact:** Eliminates one O(n) allocation and one O(n) copy per word expansion.

---

### P5. Fix pattern removal to use slices

**File:** `src/expand.rs`, `remove_parameter_pattern` (line 1235)

**Problem:** The `${var#pat}`, `${var%pat}` operators loop over the value,
building a temporary String per iteration to call `pattern_matches`.
`pattern_matches` itself allocates two `Vec<char>` per call.

**Fix:** Make `pattern_matches` work on `&str` directly (using byte-offset
iteration instead of `Vec<char>`). Pass string slices (`&value[..end]`,
`&value[start..]`) to the matcher instead of collecting into new Strings.

**Effort:** Moderate. Requires rewriting `pattern_matches` and
`remove_parameter_pattern`.

**Impact:** Reduces O(n^2) allocations to O(n) for pattern removal operations.

---

## Tier 2 — Moderate Impact, Moderate Effort

### P6. Eliminate Vec\<char\> with a Cursor abstraction

**Files:** `src/syntax.rs`, `src/expand.rs`

**Problem:** 11 sites do `chars().collect::<Vec<char>>()`, allocating 4x the
byte size of the input. The pattern exists in the tokenizer (line 476), expander
(lines 189, 340, 398, 515), arithmetic parser (line 1445), pattern matcher
(lines 1339-1340), and brace parser (line 1020).

**Fix:** Introduce a `Cursor` struct that works directly on `&str` with byte
offsets:

```rust
struct Cursor<'a> {
    src: &'a str,
    pos: usize,  // byte offset
}

impl<'a> Cursor<'a> {
    fn peek(&self) -> Option<char> { self.src[self.pos..].chars().next() }
    fn advance(&mut self) -> char { /* advance pos by char width, return char */ }
    fn slice_from(&self, start: usize) -> &'a str { &self.src[start..self.pos] }
    fn remaining(&self) -> &'a str { &self.src[self.pos..] }
}
```

Replace all `Vec<char>` + index patterns with `Cursor`. For ASCII-heavy shell
scripts (the common case), each advance is a single-byte step.

**Effort:** Moderate-to-high. Touches every scanner function in both files.

**Impact:** Eliminates 11 O(n) allocations, saves 4x memory per scan.

---

### P7. Context trait returns borrows instead of owned Strings

**File:** `src/expand.rs` (trait), `src/shell.rs` (impl)

**Problem:** Every `Context` method returns owned `String` or `Option<String>`,
forcing a clone of the variable value on every `$VAR` expansion:

```rust
fn env_var(&self, name: &str) -> Option<String>;
fn positional_params(&self) -> Vec<String>;
```

**Fix:**

```rust
fn env_var(&self, name: &str) -> Option<&str>;
fn special_param(&self, name: char) -> Option<Cow<'_, str>>;
fn positional_param(&self, index: usize) -> Option<&str>;
fn positional_params(&self) -> &[String];
```

`special_param` needs `Cow` because `$?` and `$#` are computed on the fly
(formatted from integers). Everything else can borrow from the shell's internal
storage.

**Effort:** Moderate. Requires updating trait, impl, `FakeContext`, and all call
sites in the expander.

**Impact:** Eliminates ~5 String clones per command line on average.

---

### P8. Segments use Cow to borrow from Word.raw

**File:** `src/expand.rs`

**Problem:** Each `Segment::Text(String, bool)` owns a heap String. But much of
the segment content (single-quoted strings, literal text) is a verbatim
substring of `Word.raw`.

**Fix:**

```rust
enum Segment<'a> {
    Text(Cow<'a, str>, bool),
    AtBreak,
    AtEmpty,
}
```

Literal characters and single-quoted strings borrow from `Word.raw`. Only
expanded values (`$VAR` results, command substitution output) need to be owned.

**Effort:** Moderate. Adds a lifetime to `Segment`, `ExpandedWord`, and related
functions.

**Impact:** Eliminates allocations for the non-expansion parts of words.

---

## Tier 3 — Large Effort, Architectural

### P9. Zero-copy tokenizer with span-based tokens

**File:** `src/syntax.rs`

**Problem:** `TokenKind::Word(String)` owns a copy of each word text. The parser
then clones these Strings into AST nodes (7 clone sites). `split_assignment`
creates two more Strings from an already-owned token.

**Fix option A — Cow tokens:**

```rust
enum TokenKind<'a> {
    Word(Cow<'a, str>),  // borrows source for simple words
    // ...
}
```

**Fix option B — Span-based tokens (more aggressive):**

```rust
struct Token {
    kind: TokenTag,       // enum without data
    span: (u32, u32),     // byte offsets into source
}
```

The parser constructs `Word { span: Span }` that indexes into the source. At
expansion time, resolve `&source[span.start..span.end]`.

**Trade-off:** Lifetimes propagate through `Token`, `Parser`, and all AST types.
The span approach avoids lifetimes but requires carrying the source `&str`
alongside the AST.

**Effort:** High. Touches every AST type, the parser, and the expander.

**Impact:** Eliminates all String allocations during tokenization and parsing.

---

### P10. Arena allocator for expansion temporaries

**File:** `src/expand.rs`

**Problem:** Expansion creates many short-lived Strings (segment text, field
text, expanded values) that are all discarded after `expand_word` returns.

**Fix:** Use a bump allocator. Allocate into the arena during `expand_raw` and
`split_fields_from_segments`. Reset after `expand_word` completes. The final
`Vec<String>` result copies out of the arena, but all intermediaries are
arena-allocated (O(1) alloc, bulk free).

```rust
struct ExpandArena {
    buf: Vec<u8>,
    offset: usize,
}
```

`Segment` and `Field` would hold `&'arena str` instead of `String`.

**Effort:** High. Requires lifetime management throughout the expansion pipeline.

**Impact:** Near-zero allocation cost for expansion intermediaries.

---

### P11. Replace HashMap for aliases and small variable tables

**File:** `src/shell.rs`

**Problem:** `HashMap` has significant per-entry overhead (hashing, bucket
array, pointer chasing). For small alias tables (<20 entries), this is
disproportionate.

**Fix:** Use a sorted `Vec<(String, String)>` with binary search for aliases.
For variables, consider `BTreeMap` for better cache locality, or a flat sorted
vec for very small environments.

**Effort:** Low-to-moderate.

**Impact:** Small. Reduces memory fragmentation and improves cache behavior for
small tables.

---

## Recommended Execution Order

```
Phase A — Quick wins (independent, no API changes):
  P1  Fix per-char allocation in expand_raw
  P2  Add [profile.release] to Cargo.toml
  P3  Borrow aliases instead of cloning

Phase B — Internal refactors (no public API changes):
  P4  Inline field splitting over segments
  P5  Fix pattern removal to use slices
  P6  Cursor abstraction to eliminate Vec<char>

Phase C — API evolution (lifetime threading):
  P7  Context trait returns borrows
  P8  Segments use Cow

Phase D — Architectural (optional, large effort):
  P9  Zero-copy tokenizer / span-based AST
  P10 Arena allocator for expansion
  P11 Sorted vec for small maps
```

Phase A items can be done in a single session with no risk. Phase B items are
moderate refactors that can be done incrementally. Phase C requires threading
lifetimes through the expansion engine. Phase D is optional and only worth
pursuing if benchmarks show the earlier phases are insufficient.
