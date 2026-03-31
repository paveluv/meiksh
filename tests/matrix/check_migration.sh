#!/bin/sh
# check_migration.sh — Verify .sh to .epty migration integrity.
#
# Usage: sh tests/matrix/check_migration.sh <basename> [<basename> ...]
#   e.g. sh tests/matrix/check_migration.sh job_control token_rules
#
# Checks:
#   1. Requirement IDs: sorted list with duplicates must match
#   2. Test count: number of tests in .sh must equal number in .epty

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
TESTS_DIR="$SCRIPT_DIR/tests"

errors=0

check_one() {
    name="$1"
    sh_file="$TESTS_DIR/${name}.sh"
    epty_file="$TESTS_DIR/${name}.epty"

    if [ ! -f "$sh_file" ]; then
        echo "ERROR: $sh_file not found"
        errors=$((errors + 1))
        return
    fi
    if [ ! -f "$epty_file" ]; then
        echo "ERROR: $epty_file not found"
        errors=$((errors + 1))
        return
    fi

    echo "=== $name ==="

    # --- Requirement IDs ---
    # .sh: lines like  # REQUIREMENT: SHALL-2-11-435:
    sh_reqs=$(grep -o '# REQUIREMENT: [A-Za-z0-9_-]*' "$sh_file" \
        | sed 's/# REQUIREMENT: //' | sort)

    # .epty: lines like  requirement "SHALL-2-11-435"
    epty_reqs=$(grep -o 'requirement "[^"]*"' "$epty_file" \
        | sed 's/requirement "//;s/"//' | sort)

    sh_reqs_file=$(mktemp)
    epty_reqs_file=$(mktemp)
    printf '%s\n' "$sh_reqs" > "$sh_reqs_file"
    printf '%s\n' "$epty_reqs" > "$epty_reqs_file"

    sh_req_count=$(echo "$sh_reqs" | grep -c . || true)
    epty_req_count=$(echo "$epty_reqs" | grep -c . || true)

    req_diff=$(diff "$sh_reqs_file" "$epty_reqs_file" || true)
    rm -f "$sh_reqs_file" "$epty_reqs_file"

    if [ -z "$req_diff" ]; then
        echo "  OK  requirements: $sh_req_count IDs match"
    else
        echo "  FAIL  requirements differ ($sh_req_count in .sh, $epty_req_count in .epty):"
        echo "$req_diff" | sed 's/^/        /'
        errors=$((errors + 1))
    fi

    # --- Unique requirement IDs ---
    sh_uniq_file=$(mktemp)
    epty_uniq_file=$(mktemp)
    echo "$sh_reqs" | sort -u > "$sh_uniq_file"
    echo "$epty_reqs" | sort -u > "$epty_uniq_file"

    sh_only=$(comm -23 "$sh_uniq_file" "$epty_uniq_file")
    epty_only=$(comm -13 "$sh_uniq_file" "$epty_uniq_file")
    rm -f "$sh_uniq_file" "$epty_uniq_file"

    if [ -n "$sh_only" ]; then
        echo "  WARN  requirements only in .sh:"
        echo "$sh_only" | sed 's/^/        /'
    fi
    if [ -n "$epty_only" ]; then
        echo "  WARN  requirements only in .epty:"
        echo "$epty_only" | sed 's/^/        /'
    fi

    # --- Test counts ---
    # .sh tests: assert_stdout, assert_exit_code, assert_exit_code_non_zero,
    # assert_pty_script, assert_stderr_empty, assert_stderr_contains,
    # standalone pass (from case blocks)
    sh_tests=$(grep -cE \
        '^(assert_stdout|assert_exit_code_non_zero|assert_exit_code|assert_pty_script|assert_stderr_empty|assert_stderr_contains|pass$)' \
        "$sh_file" || true)

    # .epty tests: begin test or begin interactive test
    epty_tests=$(grep -cE '^\s*begin (interactive )?test ' "$epty_file" || true)

    if [ "$sh_tests" -eq "$epty_tests" ]; then
        echo "  OK  test count: $sh_tests"
    else
        echo "  FAIL  test count: $sh_tests in .sh, $epty_tests in .epty"
        errors=$((errors + 1))
    fi

    echo
}

if [ $# -eq 0 ]; then
    echo "Usage: $0 <basename> [<basename> ...]" >&2
    echo "  e.g. $0 job_control token_rules" >&2
    exit 2
fi

for name in "$@"; do
    check_one "$name"
done

if [ "$errors" -gt 0 ]; then
    echo "FAILED: $errors error(s)"
    exit 1
else
    echo "ALL OK"
    exit 0
fi
