# Locale Handling in Meiksh

This document explains how meiksh handles locale-sensitive operations and what any code that touches characters, strings, collation, or numeric formatting must do to remain correct.

## Core Principle: Characters, Not Bytes

POSIX defines most shell operations in terms of *characters*, not bytes. In a multi-byte locale such as UTF-8, a single character may occupy 1-4 bytes. Code that iterates byte-by-byte, counts `bytes.len()`, or slices at arbitrary byte offsets is almost certainly wrong for multi-byte locales.

The rule is simple: **never assume 1 byte = 1 character**. Use the helpers in `sys::locale` instead.

## The Locale API (`sys::locale`)

All locale-sensitive operations go through function pointers on `SystemInterface` (in `sys/interface.rs`), exposed as free functions in `sys/locale.rs`. This indirection exists so unit tests can substitute deterministic mocks via the trace model.

### Character decoding and encoding

| Function | Signature | Purpose |
|---|---|---|
| `decode_char(bytes)` | `&[u8] -> (u32, usize)` | Decode one character from a byte slice. Returns `(wide_char, byte_length)`. Returns `(0, 0)` on empty input, `(byte_as_u32, 1)` on invalid sequences. Wraps `mbrtowc`. |
| `encode_char(wc)` | `u32 -> Vec<u8>` | Encode a wide character back to a byte sequence. Wraps `wcrtomb`. |
| `count_chars(bytes)` | `&[u8] -> u64` | Count the number of characters in a byte string. |
| `first_char_len(bytes)` | `&[u8] -> usize` | Byte length of the first character (minimum 1 for non-empty input). |

### Character classification and case

| Function | Signature | Purpose |
|---|---|---|
| `classify_char(class, wc)` | `(&[u8], u32) -> bool` | Test whether wide character `wc` belongs to a POSIX character class (`b"alpha"`, `b"digit"`, `b"upper"`, etc.). Wraps `iswctype` / `iswalpha` etc. |
| `to_upper(wc)` / `to_lower(wc)` | `u32 -> u32` | Case conversion for a wide character. Wraps `towupper` / `towlower`. |
| `char_width(wc)` | `u32 -> usize` | Display column width of a character. Wraps `wcwidth`. Use for cursor positioning in the line editor, not for byte counting. |

### Collation and comparison

| Function | Signature | Purpose |
|---|---|---|
| `strcoll(a, b)` | `(&[u8], &[u8]) -> Ordering` | Locale-aware string comparison. Wraps `strcoll(3)`. |

### Numeric formatting

| Function | Signature | Purpose |
|---|---|---|
| `decimal_point()` | `-> u8` | The locale's radix character (`.` in C locale, `,` in some European locales). Wraps `localeconv()->decimal_point`. |

### Locale lifecycle

| Function | Purpose |
|---|---|
| `setup_locale()` | Called once at shell startup. Calls `setlocale(LC_ALL, "")` to initialize from the environment. |
| `reinit_locale()` | Called whenever a locale variable changes at runtime. Calls `setlocale(LC_ALL, "")` again. |

## Where Locale Affects Shell Behavior

Every area listed below has been audited and fixed. New code touching any of these areas must use the locale API.

### Pattern matching (`expand/glob.rs`)

- `?` matches one *character*, not one byte. Use `decode_char` to determine the byte length of the current text character.
- `*` advances by characters, not bytes, during backtracking.
- `[...]` bracket expressions work with `u32` wide characters:
  - `[[:alpha:]]` uses `classify_char(b"alpha", wc)`.
  - Range endpoints like `[a-z]` compare decoded wide characters.
  - Collating symbols `[.ch.]` and equivalence classes `[=a=]` decode multi-byte sequences from the pattern itself.
- Characters inside the pattern string are decoded with `decode_char` (via `decode_pattern_char`), not read as single bytes.

### Parameter expansion (`expand/parameter.rs`, `expand/expand_parts.rs`)

- `${#var}` must return the *character count*, not `value.len()`. Use `locale::count_chars()`.
- `${var#pattern}` and `${var%pattern}` (prefix/suffix removal) must try pattern matches only at character boundaries. Use `char_boundary_offsets()` to enumerate valid split points, never iterate `0..len` by bytes.
- `$*` in double quotes uses the first *character* of IFS as separator, which may be multi-byte. Use `locale::first_char_len()` to extract it.

### IFS field splitting (`expand/expand_parts.rs`, `expand/word.rs`)

- IFS characters may be multi-byte. The IFS string must be decomposed into a list of locale characters using `decompose_ifs()`, which calls `decode_char` to walk the IFS value.
- When scanning expanded text for IFS delimiters, use `find_ifs_char_at()` which matches multi-byte IFS characters against the current position.
- IFS whitespace classification is strictly POSIX: only ASCII space (`0x20`), tab (`0x09`), and newline (`0x0a`) are IFS whitespace, regardless of what the locale's `isspace` says. See `is_ifs_whitespace()`.

### `printf` builtin (`builtin/printf.rs`)

- The `'X` character constant (leading single-quote) must return the `wchar_t` codepoint value of the character following the quote. Use `decode_char` to read the character, then use the `u32` wide character value directly.

### `test` builtin (`builtin/test_builtin.rs`)

- `test s1 < s2` and `test s1 > s2` compare strings using `locale::strcoll()`, not byte ordering.

### Sorted listings (`builtin/set.rs`, `builtin/alias.rs`, `expand/pathname.rs`)

- `set` variable listing, `alias` listing, and glob result ordering all sort using `locale::strcoll()`, not `sort()` / byte comparison.

### `fc` builtin (`builtin/fc.rs`)

- `fc -s old=new` must find the `old` substring at character boundaries only, to avoid splitting a multi-byte character. Use `find_on_char_boundary()`.

### `getopts` builtin (`builtin/getopts.rs`)

- Option characters may be multi-byte. The optstring and the argument string are scanned with `decode_char`, and option matching compares `u32` wide characters.

### `time` / `times` output (`bstr.rs`)

- Decimal point in floating-point output uses `locale::decimal_point()` (LC_NUMERIC) instead of a hardcoded `b'.'`.

### Vi line editor (`interactive/vi_editing.rs`)

- Cursor movement (`h`, `l`) steps by character length using `char_len_at` / `prev_char_start`.
- Backspace / delete (`x`, `X`) removes full multi-byte characters.
- Case toggle (`~`) decodes, converts, re-encodes, and handles potential byte-length changes.
- Display-width calculations use `char_width(wc)` for correct cursor positioning with wide/zero-width characters.
- Word classification (`is_word_char`) uses `classify_char(b"alnum", wc)`.

### `read` builtin

- IFS field splitting in `read` uses the same character-aware decomposition as word expansion.

## Runtime Locale Changes

The shell must respond to locale variable changes at runtime. When a script does `export LC_ALL=C.UTF-8`, subsequent character operations must reflect the new locale.

This is implemented in `shell/env.rs`:

1. `set_var` and `unset_var` check whether the variable name is a locale variable (`LC_ALL`, `LC_CTYPE`, `LC_COLLATE`, `LC_NUMERIC`, `LC_MESSAGES`, `LC_TIME`, `LANG`).
2. If it is, they call `sys::env::env_set_var` / `env_unset_var` to synchronize the OS `environ` (so the C library can see it).
3. Then they call `sys::locale::reinit_locale()` which calls `setlocale(LC_ALL, "")` to make the C library re-read the environment.

The `#[cfg(not(test))]` guard around the `env_set_var` / `env_unset_var` calls prevents unit tests (which use the mock `SystemInterface`) from calling into the real C library. See the testing section below.

## PATH Cache Invalidation

Though not locale-related, PATH interacts with the same `set_var` / `unset_var` hooks: when `PATH` changes, `path_cache` is cleared. This also applies to `restore_vars` in `exec/simple.rs` after temporary command-prefix assignments like `PATH=/foo cmd`.

## Common Patterns

### Iterating over characters in a byte string

```rust
let mut i = 0;
while i < bytes.len() {
    let (wc, len) = locale::decode_char(&bytes[i..]);
    let step = if len == 0 { 1 } else { len };
    // wc is the u32 wide character, step is its byte length
    // ... do something with wc ...
    i += step;
}
```

### Iterating at character boundaries (for split points)

```rust
fn char_boundary_offsets(value: &[u8]) -> Vec<usize> {
    let mut offsets = vec![0];
    let mut i = 0;
    while i < value.len() {
        let (_, len) = locale::decode_char(&value[i..]);
        i += if len == 0 { 1 } else { len };
        offsets.push(i);
    }
    offsets
}
```

### Sorting strings by collation

```rust
items.sort_by(|a, b| locale::strcoll(a, b));
```

### Checking character class membership

```rust
let (wc, len) = locale::decode_char(&text[pos..]);
if locale::classify_char(b"alpha", wc) {
    // wc is alphabetic in the current locale
}
```

## Unit Testing

The `SystemInterface` locale function pointers are mocked in `sys/test_support.rs`. The trace implementations (`trace_decode_char`, `trace_classify_char`, etc.) provide UTF-8-aware behavior using Rust's built-in Unicode tables, without calling into the C library.

Several `sys::locale` functions have `#[cfg(test)]` fallback blocks that return ASCII/byte-oriented defaults when no test interface is active. This prevents panics in unit tests that exercise code paths calling locale functions but don't set up the full trace infrastructure. Specifically:

- `decode_char` falls back to `(bytes[0] as u32, 1)` — single-byte decoding.
- `classify_char` falls back to ASCII-only classification via `classify_byte`.
- `strcoll` falls back to byte comparison.
- `decimal_point` falls back to `b'.'`.
- `reinit_locale` becomes a no-op.

If your unit test specifically tests multi-byte behavior, use `run_trace` with a trace table that includes the expected locale calls. For integration and matrix tests, the real C library handles locale correctly.

## Matrix Test Coverage

The `tests/matrix/tests/` directory contains `expect_pty` test suites that exercise locale behavior as a black box. Each locale-sensitive area listed above has at least one corresponding matrix test. Tests that need UTF-8 set `export LC_ALL=C.UTF-8` as the first line of the `script` block. Tests in the C locale rely on the `expect_pty` default environment (`LC_ALL=C`).

Key test files:
- `2_14_pattern_matching_notation.md` — multi-byte `?`, `*`, `[[:alpha:]]`
- `2_6_word_expansions.md` — `${#var}` character count, multi-byte IFS splitting, `$*` separator, trim at character boundaries
- `2_5_parameters_and_variables.md` — runtime locale change affecting pattern matching and string length
- `xbd_8_environment_variables.md` — PATH cache invalidation
- `maybe_builtin_printf.md` — `printf 'é` codepoint value
- `maybe_builtin_test.md` — `test <`/`>` collation
- `intrinsic_utility_alias.md` — alias listing sort order
- `xbd_9_3_5_re_bracket_expression.md` — equivalence classes

## Checklist for New Code

Before merging any code that handles text, strings, or characters:

1. Does it iterate over a byte string? If yes, it must step by `decode_char` character length, not by single bytes.
2. Does it count string length? Use `count_chars()`, not `.len()`.
3. Does it split a string at an offset? The offset must be at a character boundary.
4. Does it compare or sort strings? Use `strcoll()` if POSIX requires locale-aware ordering.
5. Does it classify characters (alphabetic, digit, etc.)? Use `classify_char()` with the POSIX class name.
6. Does it do case conversion? Use `to_upper()` / `to_lower()` on decoded `u32` wide characters.
7. Does it format numbers? Use `decimal_point()` for the radix character.
8. Does it need display width for terminal output? Use `char_width()`.
9. Does it modify a locale environment variable? The `set_var` / `unset_var` hooks handle this automatically; don't bypass them.
10. Does it have a matrix test? Every locale-sensitive feature needs black-box test coverage.
