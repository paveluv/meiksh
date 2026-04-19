# Performance Diagnosis: Why meiksh Is Slower

## Answer: It's Not Redundant Forking

Meiksh's fork/exec behavior is correct — it matches what other shells do:
- Builtins run in-process (no fork)
- Functions run in-process (no fork)
- `$(...)` always forks (required to capture stdout via pipe) — same as every other shell
- Multi-command pipelines fork each stage — same as every other shell
- Single-command pipelines skip the pipeline-level fork — correct

The 2-6x slowdown vs dash is entirely **in-process overhead** in the interpreter's hot loops.

## Root Causes (ranked by impact)

### 1. Function body deep-cloned on every call (CRITICAL)

In `src/exec/simple.rs` line 206:

```rust
if let Some(function) = shell.functions.get(&owned_argv[0]).cloned() {
```

`.cloned()` deep-copies the entire `Command` AST — every `Vec<ListItem>`, `Vec<Word>`, `Vec<u8>` — on every function invocation. For `fib_iter` called 6,400 times with the loop body containing 5+ nodes, this is an allocation storm.

**Fix:** Borrow the AST via `Rc<Command>` or restructure to use `&Command` reference. The clone exists because the borrow of `shell.functions` conflicts with the mutable borrow needed to execute. A common solution is `Rc::clone()` (cheap reference count bump) or extracting the function body before the mutable borrow.

### 2. Trap action strings re-parsed from text every execution (CRITICAL)

In `src/shell/traps.rs`, `TrapAction` stores the action as raw `Vec<u8>` text. When executed, `src/shell/run.rs` calls `self.execute_string(action)` which invokes the full parser on the text. The benchmark does `trap ":" USR1; trap - USR1` 307,000 times — each `trap ":" USR1` parses and stores the string `":"`, and each trap fire would re-parse it.

Additionally, each trap set/reset does two `sigaction` syscalls (install handler + reset to default).

**Fix:** Pre-parse trap actions into AST at `trap` set time and store `Option<Program>` alongside the text. Execute the cached AST instead of re-parsing. The syscalls are inherent.

### 3. Arithmetic re-parses expression text every evaluation (HIGH)

`src/expand/arithmetic.rs` creates a fresh `ArithmeticParser` on every `$(( expr ))`. Each evaluation:
- Allocates a `Vec<u8>` for the expanded expression text
- Allocates a `Vec<u8>` per variable name (`try_scan_name` clones the name slice)
- Allocates a `Vec<u8>` for the trimmed value
- Allocates a `Vec<u8>` for the `i64_to_bytes` result

In the arithmetic benchmark, `i=$(( i + 1 ))` hits this path 196,300 times. That's ~800,000+ small allocations just for the arithmetic.

**Fix:** Avoid allocations in the hot path — use stack buffers for small integers, borrow variable names instead of cloning, return `i64` directly instead of converting to `Vec<u8>` and back.

### 4. `expand_simple` overhead per command execution (HIGH)

Every simple command execution in `src/exec/simple.rs` does:
- `ByteArena::new()` — a fresh arena per command
- `expand_words()` — builds `Vec<Segment>` with `Vec<u8>` per segment
- Field splitting materializes `Vec<(u8, QuoteState)>` for the entire input
- `owned_argv: Vec<Vec<u8>>` and `owned_assignments` clone all words

In a tight `while` loop with `[ $i -lt N ]` and `i=$(( i + 1 ))`, this expansion machinery runs twice per iteration — once for `[ ... ]` and once for the assignment.

**Fix:** This is the deepest structural issue. Options include: arena-based allocation that persists across the loop, small-string optimization, or reducing intermediate allocations in the expand path.

### 5. Field splitting materializes the full byte stream (HIGH)

In `src/expand/model.rs`, `split_fields_from_segments` calls:
```rust
let chars: Vec<(u8, QuoteState)> = segment_bytes(segments).collect();
```
This materializes every byte as a `(u8, QuoteState)` tuple into a heap-allocated `Vec`. For long values, this is O(n) allocation for what should be a streaming single-pass scan.

**Fix:** Iterate over `segment_bytes(segments)` directly instead of collecting into a Vec.

### 6. `$?`, `$#`, `$-` allocate on every reference (MEDIUM)

In `src/shell/expand_context.rs`, `special_param(b'?')` calls `bstr::i64_to_bytes(self.last_status)` which allocates a new `Vec<u8>` every time `$?` is referenced. In arithmetic loops, `$?` is checked frequently.

**Fix:** Cache the string representation of `last_status` and invalidate on change, or use a stack-allocated buffer.

## What This Means

The core issue is that meiksh's interpreter pays Rust's allocation cost on every operation in a tight loop, whereas C shells like dash use direct pointer manipulation, stack allocation, and in-place mutation. The AST is not re-parsed per iteration (good), but the execution of each AST node involves significant transient heap allocation.

The highest-impact fixes would be:
1. **Eliminate the function body clone** — likely recovers most of the function-call gap
2. **Pre-parse trap actions** — eliminates re-parsing overhead
3. **Reduce allocations in arithmetic** — stack buffers for small integers
4. **Stream field splitting** — avoid materializing the byte vector
5. **Arena-persistent allocation** in the expand/execute hot path
