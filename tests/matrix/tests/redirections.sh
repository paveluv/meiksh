# Test: Redirections
# Target: tests/matrix/tests/redirections.sh
#
# Ah, Redirections! The plumbing of Unix pipelines. Here we verify that the
# shell expertly redirects stdout, stderr, stdin, and correctly sets up file
# descriptors according to POSIX standards. Every test here runs in an isolated
# temporary directory, so we don't need to fear the dreaded `rm -rf`.

. "$MATRIX_DIR/lib.sh"


# ==============================================================================
# The Gateway: Basic Redirections
# ==============================================================================
# REQUIREMENT: SHALL-2-7-0-050: Redirection operators: Redirection is used to
# open and close files for the current shell execution environment

# We pipe raw output straight to a file named `tmp.txt` using the standard `>`
# redirection. Then we eagerly scoop it back up with `cat` to verify it landed.
test_cmd="echo hello > tmp.txt; cat tmp.txt"
assert_stdout "hello" \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# Reading Files: Input Redirection
# ==============================================================================
# REQUIREMENT: SHALL-2-7-1-055: Input Redirection: The general format for
# redirecting input is: [n]<word
# REQUIREMENT: SHALL-2-7-1-198: If the number is omitted, the redirection shall
# refer to standard input (file descriptor 0).

# Reading files via `<` attaches the specified file directly to the stdin of
# the command. We verify `cat` reads exactly what we injected into it.
test_cmd="echo world > tmp.txt; cat < tmp.txt"
assert_stdout "world" \
    "$TARGET_SHELL -c '$test_cmd'"

# We also explicitly test `<` mapped to stdin using `0<`.
test_cmd="echo world > tmp.txt; cat 0< tmp.txt"
assert_stdout "world" \
    "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-2-7-190: If n is quoted, the number shall not be recognized
# as part of the redirection expression.

# If we quote the number before the redirection operator (e.g. `'0'< file`), it
# is treated as a literal command name `0` rather than file descriptor 0.
test_cmd='echo content > tmp.txt; "0"<tmp.txt'
assert_exit_code_non_zero \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# Writing Files: Output Redirection
# ==============================================================================
# REQUIREMENT: SHALL-2-7-2-060: Output Redirection: The general format for
# redirecting output is: [n]>word
# REQUIREMENT: SHALL-2-7-2-199: If the number is omitted, the redirection shall
# refer to standard output (file descriptor 1).

# By specifying `1>`, we tell the shell to explicitly map File Descriptor 1
# (stdout) to our file. It should behave identically to the implicit `>` above.
test_cmd="echo foo 1> tmp.txt; cat tmp.txt"
assert_stdout "foo" \
    "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-2-7-2-200: Output redirection using the '>' format shall
# fail if the noclobber option is set.
# REQUIREMENT: SHALL-2-7-2-202: In all other cases (noclobber not set),
# redirection using '>' does not fail for the reasons stated above.

# When `set -C` (noclobber) is enabled, the shell must refuse to overwrite an
# existing file.
test_cmd="set -C; echo a > tmp.txt; echo b > tmp.txt"
assert_exit_code_non_zero \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# Appending Output
# ==============================================================================
# REQUIREMENT: SHALL-2-7-3-204: Appended output redirection shall cause the file
# whose name results from the expansion of word to be opened for appending...
# REQUIREMENT: SHALL-2-7-3-206: If the file does not exist, it shall be created.

# We test appending to a new file, and then appending to the existing file.
test_cmd="echo a >> tmp_append.txt; echo b >> tmp_append.txt;
cat tmp_append.txt"
assert_stdout "a
b" \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# In-Band Data: Here-Documents
# ==============================================================================
# REQUIREMENT: SHALL-2-7-4-065: Here-Document: The redirection operator << is
# used to read input from the current source file...
# REQUIREMENT: SHALL-2-7-4-210: The delimiter shall be the word itself.

# A here-document feeds a multiline string directly into a command's stdin.
# We ensure the shell successfully captures the delimiter `EOF` and processes
# the entire block as a contiguous stream.
test_cmd="cat <<EOF
line1
line2
EOF"

expected_output="line1
line2"

assert_stdout "$expected_output" \
    "$TARGET_SHELL -c \"\$test_cmd\""

# REQUIREMENT: SHALL-2-7-4-209: If any part of word is quoted... the delimiter
# shall be the word formed by quote removal...
# REQUIREMENT: SHALL-2-7-4-214: If no part of word is quoted, all lines of the
# here-document shall be expanded for parameter expansion...

# Testing expansions in here-documents.
test_cmd="var=hello
cat <<EOF
\$var
EOF"
assert_stdout "hello" \
    "$TARGET_SHELL -c \"\$test_cmd\""

# Testing quoted delimiters in here-documents.
test_cmd="var=hello
cat <<'EOF'
\$var
EOF"
assert_stdout '$var' \
    "$TARGET_SHELL -c \"\$test_cmd\""


report
