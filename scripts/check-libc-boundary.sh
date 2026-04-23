#!/bin/sh
# Enforce the libc boundary defined in docs/IMPLEMENTATION_POLICY.md:
#   libc is only permitted in:
#     1. src/sys/**
#     2. tests/integration/sys.rs
#     3. tests/expect_pty.rs (standalone matrix-test driver binary)
#
# Fails with a non-zero exit code and a list of offenders if any other file
# contains a real `libc::` token, `use libc` statement, or `extern crate libc`.
# Lines that are comments are ignored.

set -eu

repo_root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
cd "$repo_root"

if ! command -v rg >/dev/null 2>&1; then
    echo "check-libc-boundary.sh: requires ripgrep (rg)" >&2
    exit 2
fi

# Match libc references on non-comment lines. The leading assertion strips
# lines whose first non-whitespace content starts with `//`.
pattern='^(?![[:space:]]*//).*(\blibc::|^[[:space:]]*use[[:space:]]+libc\b|extern[[:space:]]+crate[[:space:]]+libc)'

matches=$(rg -l --pcre2 --glob '!target/**' --glob '*.rs' \
    -e "$pattern" \
    . 2>/dev/null || true)

violations=""
for f in $matches; do
    case "$f" in
        src/sys/*|./src/sys/*)
            ;;
        tests/integration/sys.rs|./tests/integration/sys.rs)
            ;;
        tests/expect_pty.rs|./tests/expect_pty.rs)
            ;;
        *)
            violations="$violations $f"
            ;;
    esac
done

if [ -z "$violations" ]; then
    echo "check-libc-boundary.sh: OK (libc confined to permitted modules)"
    exit 0
fi

echo "check-libc-boundary.sh: libc used outside permitted modules:" >&2
for f in $violations; do
    echo "  --- $f" >&2
    rg -n --no-heading --pcre2 --glob '*.rs' -e "$pattern" "$f" >&2 || true
done
echo "" >&2
echo "Per docs/IMPLEMENTATION_POLICY.md (Low-Level Interface Boundary):" >&2
echo "  - Production libc lives only in src/sys/**." >&2
echo "  - Test-only libc lives only in tests/integration/sys.rs." >&2
echo "  - The standalone matrix-test driver tests/expect_pty.rs is also permitted." >&2
echo "  - All other modules must go through those wrappers." >&2
exit 1
