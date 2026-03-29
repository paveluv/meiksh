# Test: pwd — Print Working Directory
# Target: tests/matrix/tests/pwd_extended.sh
#
# POSIX compliance tests for the pwd utility covering -L and -P options,
# pathname format, symlink handling, and error behavior.

. "$MATRIX_DIR/lib.sh"

_tmpdir="${TMPDIR:-/tmp}/pwd_test_$$"
mkdir -p "$_tmpdir/real/deep"
ln -sfn "$_tmpdir/real/deep" "$_tmpdir/link"

# ==============================================================================
# -L option: use PWD if it is an absolute pathname with no dot/dot-dot/symlink
# ==============================================================================
# REQUIREMENT: SHALL-OPTIONS-5055:
# -L option: if the PWD environment variable contains an absolute pathname
# of the current directory that does not contain the filenames dot or
# dot-dot or symlink components, pwd shall write this to stdout.

# When PWD is set to the real path (no symlinks), -L should use it.
_real=$($TARGET_SHELL -c "cd '$_tmpdir/real/deep' && pwd -P" 2>/dev/null)
_out=$($TARGET_SHELL -c "cd '$_tmpdir/real/deep' && PWD='$_real' pwd -L" 2>/dev/null)
if [ "$_out" = "$_real" ]; then
    pass
else
    fail "pwd -L with clean PWD expected '$_real', got '$_out'"
fi

# PWD containing ".." should NOT be trusted by -L; should fall back.
_out=$($TARGET_SHELL -c "cd '$_tmpdir/real/deep' && PWD='$_tmpdir/real/deep/../deep' pwd -L" 2>/dev/null)
_physical=$($TARGET_SHELL -c "cd '$_tmpdir/real/deep' && pwd -P" 2>/dev/null)
case "$_out" in
    *".."*) fail "pwd -L should not output a path with '..' components, got '$_out'" ;;
    *) pass ;;
esac

# ==============================================================================
# -L falls back to -P when PWD is invalid
# ==============================================================================
# REQUIREMENT: SHALL-OPTIONS-5056:
# Otherwise, -L behaves as -P.

# If PWD is set to a bogus value, -L should fall back to physical path.
_physical=$($TARGET_SHELL -c "cd '$_tmpdir/real/deep' && pwd -P" 2>/dev/null)
_out=$($TARGET_SHELL -c "cd '$_tmpdir/real/deep' && PWD=/nonexistent/bogus pwd -L" 2>/dev/null)
if [ "$_out" = "$_physical" ]; then
    pass
else
    fail "pwd -L with bogus PWD expected physical '$_physical', got '$_out'"
fi

# If PWD contains a symlink component, -L should resolve or fall back.
_out=$($TARGET_SHELL -c "cd '$_tmpdir/real/deep' && PWD='$_tmpdir/link' pwd -L" 2>/dev/null)
# The output should either be the logical (symlink) path or the physical path,
# but it must be a valid absolute path that refers to the same directory.
case "$_out" in
    /*) pass ;;
    *) fail "pwd -L output should be absolute, got '$_out'" ;;
esac

# ==============================================================================
# Multiple pathnames: one starting with / and one with //
# ==============================================================================
# REQUIREMENT: SHALL-OPTIONS-5058:
# Multiple pathnames possible: one starting with single / and one with //.
# If the system distinguishes // from /, pwd might return either form.

# pwd output must start with / (absolute pathname).
_out=$($TARGET_SHELL -c 'pwd -P' 2>/dev/null)
case "$_out" in
    /*) pass ;;
    *) fail "pwd -P should output absolute path starting with /, got '$_out'" ;;
esac

_out=$($TARGET_SHELL -c 'pwd -L' 2>/dev/null)
case "$_out" in
    /*) pass ;;
    *) fail "pwd -L should output absolute path starting with /, got '$_out'" ;;
esac

# Default (no option) also absolute.
_out=$($TARGET_SHELL -c 'pwd' 2>/dev/null)
case "$_out" in
    /*) pass ;;
    *) fail "pwd (no opts) should output absolute path starting with /, got '$_out'" ;;
esac

# ==============================================================================
# Pathname shall not contain unnecessary slash characters
# ==============================================================================
# REQUIREMENT: SHALL-OPTIONS-5059:
# The pathname written to standard output shall not contain any unnecessary
# <slash> characters after the leading one (or two, if the implementation
# distinguishes //).

# pwd -P output should not have double slashes (except possibly leading //).
_out=$($TARGET_SHELL -c "cd / && pwd -P" 2>/dev/null)
_stripped=$(echo "$_out" | sed 's|^//||')
case "$_stripped" in
    *//*) fail "pwd -P contains unnecessary double slashes: '$_out'" ;;
    *) pass ;;
esac

_out=$($TARGET_SHELL -c "cd '$_tmpdir/real/deep' && pwd -P" 2>/dev/null)
_stripped=$(echo "$_out" | sed 's|^//||')
case "$_stripped" in
    *//*) fail "pwd -P contains unnecessary double slashes: '$_out'" ;;
    *) pass ;;
esac

# pwd -L output should also be clean.
_out=$($TARGET_SHELL -c "cd '$_tmpdir/real/deep' && pwd -L" 2>/dev/null)
_stripped=$(echo "$_out" | sed 's|^//||')
case "$_stripped" in
    *//*) fail "pwd -L contains unnecessary double slashes: '$_out'" ;;
    *) pass ;;
esac

# Pathname should not have a trailing slash (unless it is the root directory).
_out=$($TARGET_SHELL -c "cd '$_tmpdir/real/deep' && pwd -P" 2>/dev/null)
case "$_out" in
    */) fail "pwd -P output should not have trailing slash: '$_out'" ;;
    *) pass ;;
esac

# ==============================================================================
# Output format: "%s\n", <directory pathname>
# ==============================================================================
# REQUIREMENT: SHALL-STDERR-5063:
# Output format shall be: "%s\n", <directory pathname>.
# Exactly one newline after the pathname.

# Verify output is exactly the pathname plus a single newline.
_out=$($TARGET_SHELL -c 'pwd' 2>/dev/null)
_lines=$(echo "$_out" | wc -l | tr -d ' ')
if [ "$_lines" = "1" ]; then
    pass
else
    fail "pwd should produce exactly 1 line, got $_lines lines"
fi

# Verify pwd output ends with a newline (wc -c on pwd output vs printf without newline).
_raw_len=$($TARGET_SHELL -c 'pwd' 2>/dev/null | wc -c | tr -d ' ')
_path_len=$(printf '%s' "$($TARGET_SHELL -c 'pwd' 2>/dev/null)" | wc -c | tr -d ' ')
_diff=$((_raw_len - _path_len))
if [ "$_diff" = "1" ]; then
    pass
else
    fail "pwd output should have exactly 1 trailing newline, diff=$_diff"
fi

# Stderr should be empty on success.
assert_stderr_empty "$TARGET_SHELL -c 'pwd'"
assert_stderr_empty "$TARGET_SHELL -c 'pwd -L'"
assert_stderr_empty "$TARGET_SHELL -c 'pwd -P'"

# ==============================================================================
# Consequences of errors: no output on error (other than write error)
# ==============================================================================
# REQUIREMENT: SHALL-CONSEQUENCES-OF-ERRORS-5064:
# On error (other than write error), no output to stdout shall be produced.

# If the current directory is deleted, pwd should either still succeed
# (cached) or fail with no stdout and non-zero exit.
_errdir="$_tmpdir/will_remove"
mkdir -p "$_errdir"
_result=$($TARGET_SHELL -c "cd '$_errdir' && rmdir '$_errdir' && pwd -P" 2>/dev/null)
_rc=$?
if [ "$_rc" -eq 0 ]; then
    # Shell may cache the directory; if it succeeds, it must still print a path.
    case "$_result" in
        /*) pass ;;
        *) fail "pwd -P succeeded but did not output an absolute path: '$_result'" ;;
    esac
else
    # If it failed, stdout must be empty.
    if [ -z "$_result" ]; then
        pass
    else
        fail "pwd -P failed (rc=$_rc) but produced stdout: '$_result'"
    fi
fi

# Clean up.
rm -rf "$_tmpdir"

report
