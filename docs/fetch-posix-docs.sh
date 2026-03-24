#!/bin/sh

set -e

DOCS_DIR="$(dirname "$0")"
POSIX_DIR="$DOCS_DIR/posix"

echo "Creating directories..."
mkdir -p "$POSIX_DIR"/{issue7,issue8,utilities,functions,validation}

echo "Fetching main shell documents..."
curl -LfsS "https://pubs.opengroup.org/onlinepubs/9799919799/utilities/V3_chap02.html" -o "$POSIX_DIR/issue8/shell-command-language.html"
curl -LfsS "https://pubs.opengroup.org/onlinepubs/9799919799/utilities/sh.html" -o "$POSIX_DIR/issue8/sh-utility.html"
curl -LfsS "https://pubs.opengroup.org/onlinepubs/9799919799/xrat/V4_xcu_chap01.html" -o "$POSIX_DIR/issue8/shell-rationale.html"

curl -LfsS "https://pubs.opengroup.org/onlinepubs/9699919799/utilities/V3_chap02.html" -o "$POSIX_DIR/issue7/shell-command-language.html"
curl -LfsS "https://pubs.opengroup.org/onlinepubs/9699919799/utilities/sh.html" -o "$POSIX_DIR/issue7/sh-utility.html"

curl -LfsS "https://pubs.opengroup.org/onlinepubs/9799919799/utilities/contents.html" -o "$POSIX_DIR/issue8/contents.html"
curl -LfsS "https://pubs.opengroup.org/onlinepubs/9699919799/utilities/contents.html" -o "$POSIX_DIR/issue7/contents.html"

echo "Fetching shell-related utility pages..."
for spec in alias bg break cd command continue dot eval exec exit export fg jobs pwd read readonly return set shift times trap umask unalias unset wait; do
  echo "  - $spec"
  curl -LfsS "https://pubs.opengroup.org/onlinepubs/9799919799/utilities/${spec}.html" -o "$POSIX_DIR/utilities/${spec}.html"
done

echo "Fetching shell-related function pages..."
for func in close dup dup2 exec fork isatty kill open pipe setpgid sigaction tcgetpgrp tcsetpgrp wait waitid waitpid wordexp; do
  echo "  - $func"
  curl -LfsS "https://pubs.opengroup.org/onlinepubs/9799919799/functions/${func}.html" -o "$POSIX_DIR/functions/${func}.html"
done

echo "Fetching shell chapter/index pages..."
curl -LfsS "https://pubs.opengroup.org/onlinepubs/9799919799/utilities/V3_chap01.html" -o "$POSIX_DIR/utilities/V3_chap01.html"
curl -LfsS "https://pubs.opengroup.org/onlinepubs/9799919799/utilities/V3_chap02.html" -o "$POSIX_DIR/utilities/V3_chap02.html"
curl -LfsS "https://pubs.opengroup.org/onlinepubs/9799919799/utilities/V3_chap03.html" -o "$POSIX_DIR/utilities/V3_chap03.html"
curl -LfsS "https://pubs.opengroup.org/onlinepubs/9799919799/utilities/contents.html" -o "$POSIX_DIR/utilities/contents.html"
curl -LfsS "https://pubs.opengroup.org/onlinepubs/9799919799/functions/V2_chap02.html" -o "$POSIX_DIR/functions/V2_chap02.html"
curl -LfsS "https://pubs.opengroup.org/onlinepubs/9799919799/utilities/wait.html" -o "$POSIX_DIR/utilities/wait.html"
curl -LfsS "https://pubs.opengroup.org/onlinepubs/9799919799/functions/waitpid.html" -o "$POSIX_DIR/functions/waitpid.html"

echo "Fetching the validation reference..."
curl -LfsS "https://pubs.opengroup.org/onlinepubs/9799919799/" -o "$POSIX_DIR/validation/posix-test-suites.html"

echo "Done! All documents have been fetched to $POSIX_DIR"
