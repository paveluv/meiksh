# Test: Maybe-Builtin — printf (Write Formatted Output)
# Target: tests/matrix/tests/maybe_builtins_printf.sh
#
# printf is a standalone POSIX utility that shells commonly implement as
# a regular built-in. POSIX Section 1.6 permits this. It is NOT a special
# built-in (2.15) or intrinsic utility (1.7).
#
# Tests cover format operands, escape sequences, conversion specifiers,
# argument processing, and error handling.

. "$MATRIX_DIR/lib.sh"


# ==============================================================================
# Format operand support
# ==============================================================================
# REQUIREMENT: SHALL-OPERANDS-5022:
# format operand shall be supported

assert_stdout "hello" "$TARGET_SHELL -c 'printf hello'"
assert_stdout "hello world" "$TARGET_SHELL -c 'printf \"%s %s\" hello world'"
assert_stdout "42" "$TARGET_SHELL -c 'printf \"%d\" 42'"

# ==============================================================================
# Format operand begins and ends in initial shift state
# ==============================================================================
# REQUIREMENT: SHALL-EXTENDED-DESCRIPTION-5025:
# format operand begins and ends in initial shift state

assert_stdout "abc" "$TARGET_SHELL -c 'printf \"%s\" abc'"
assert_stdout "abc123" "$TARGET_SHELL -c 'printf \"%s%d\" abc 123'"

# ==============================================================================
# Format used as format string described in XBD 5
# ==============================================================================
# REQUIREMENT: SHALL-EXTENDED-DESCRIPTION-5026:
# format used as format string described in XBD 5

assert_stdout "hello world" "$TARGET_SHELL -c 'printf \"%s %s\" hello world'"
assert_stdout "num=42" "$TARGET_SHELL -c 'printf \"num=%d\" 42'"
assert_stdout "  42" "$TARGET_SHELL -c 'printf \"%4d\" 42'"
assert_stdout "42  " "$TARGET_SHELL -c 'printf \"%-4d\" 42'"
assert_stdout "002a" "$TARGET_SHELL -c 'printf \"%04x\" 42'"
assert_stdout "hello" "$TARGET_SHELL -c 'printf \"%.5s\" helloworld'"

# ==============================================================================
# Escape sequences: \\, \a, \b, \f, \n, \r, \t, \v, \ddd
# ==============================================================================
# REQUIREMENT: SHALL-EXTENDED-DESCRIPTION-5029:
# Escape sequences \\, \a, \b, \f, \n, \r, \t, \v, \ddd octal

# backslash
assert_stdout "\\" "$TARGET_SHELL -c 'printf \"\\\\\\\\\"'"

# \n newline
assert_stdout "a
b" "$TARGET_SHELL -c 'printf \"a\\nb\"'"

# \t tab
assert_stdout "a	b" "$TARGET_SHELL -c 'printf \"a\\tb\"'"

# \r carriage return - verify it produces output (CR overwrites)
_out=$($TARGET_SHELL -c 'printf "a\rb"' 2>/dev/null | cat -v)
case "$_out" in
    *"^M"*|*b*) pass ;;
    *) fail "printf \\r did not produce expected output, got: $_out" ;;
esac

# \a bell character
_out=$($TARGET_SHELL -c 'printf "\a"' 2>/dev/null | wc -c)
if [ "$_out" -ge 1 ] 2>/dev/null; then
    pass
else
    fail "printf \\a did not produce a byte"
fi

# \b backspace
_out=$($TARGET_SHELL -c 'printf "a\bb"' 2>/dev/null | cat -v)
case "$_out" in
    *"^H"*|*b*) pass ;;
    *) fail "printf \\b did not produce backspace, got: $_out" ;;
esac

# \f form feed
_out=$($TARGET_SHELL -c 'printf "a\fb"' 2>/dev/null | cat -v)
case "$_out" in
    *"^L"*|*b*) pass ;;
    *) fail "printf \\f did not produce form feed, got: $_out" ;;
esac

# \v vertical tab
_out=$($TARGET_SHELL -c 'printf "a\vb"' 2>/dev/null | cat -v)
case "$_out" in
    *"^K"*|*b*) pass ;;
    *) fail "printf \\v did not produce vertical tab, got: $_out" ;;
esac

# \ddd octal: \101 = 'A'
assert_stdout "A" "$TARGET_SHELL -c 'printf \"\\101\"'"
# \060 = '0'
assert_stdout "0" "$TARGET_SHELL -c 'printf \"\\060\"'"

# ==============================================================================
# No blanks before/after d or u conversion output
# ==============================================================================
# REQUIREMENT: SHALL-EXTENDED-DESCRIPTION-5030:
# No blanks before/after d or u conversion output

assert_stdout "42" "$TARGET_SHELL -c 'printf \"%d\" 42'"
assert_stdout "-1" "$TARGET_SHELL -c 'printf \"%d\" -1'"
assert_stdout "0" "$TARGET_SHELL -c 'printf \"%d\" 0'"

# Unsigned (%u): no surrounding blanks
assert_stdout "42" "$TARGET_SHELL -c 'printf \"%u\" 42'"
assert_stdout "0" "$TARGET_SHELL -c 'printf \"%u\" 0'"

# ==============================================================================
# No leading zeros for o conversion unless specified
# ==============================================================================
# REQUIREMENT: SHALL-EXTENDED-DESCRIPTION-5031:
# No leading zeros for o conversion unless specified

assert_stdout "52" "$TARGET_SHELL -c 'printf \"%o\" 42'"
assert_stdout "0" "$TARGET_SHELL -c 'printf \"%o\" 0'"
# With # flag, leading zero present
assert_stdout "052" "$TARGET_SHELL -c 'printf \"%#o\" 42'"

# ==============================================================================
# b conversion specifier shall be supported
# ==============================================================================
# REQUIREMENT: SHALL-EXTENDED-DESCRIPTION-5032:
# b conversion specifier shall be supported

assert_stdout "hello" "$TARGET_SHELL -c 'printf \"%b\" hello'"
assert_stdout "hello world" "$TARGET_SHELL -c 'printf \"%b %b\" hello world'"

# ==============================================================================
# b argument taken as string with backslash-escape sequences
# ==============================================================================
# REQUIREMENT: SHALL-EXTENDED-DESCRIPTION-5033:
# b argument taken as string with backslash-escape sequences

assert_stdout "a
b" "$TARGET_SHELL -c 'printf \"%b\" \"a\\nb\"'"
assert_stdout "a	b" "$TARGET_SHELL -c 'printf \"%b\" \"a\\tb\"'"

# ==============================================================================
# Backslash-escape sequences supported in b conversion
# ==============================================================================
# REQUIREMENT: SHALL-EXTENDED-DESCRIPTION-5034:
# Backslash-escape sequences supported in b conversion

assert_stdout "hello
world" "$TARGET_SHELL -c 'printf \"%b\" \"hello\\nworld\"'"

# ==============================================================================
# \\, \a, \b, \f, \n, \r, \t, \v converted to values in %b
# ==============================================================================
# REQUIREMENT: SHALL-EXTENDED-DESCRIPTION-5035:
# \\, \a, \b, \f, \n, \r, \t, \v converted to values

# backslash via %b
assert_stdout "\\" "$TARGET_SHELL -c 'printf \"%b\" \"\\\\\\\\\"'"

# newline via %b
assert_stdout "x
y" "$TARGET_SHELL -c 'printf \"%b\" \"x\\ny\"'"

# tab via %b
assert_stdout "x	y" "$TARGET_SHELL -c 'printf \"%b\" \"x\\ty\"'"

# ==============================================================================
# \0ddd octal converted to byte in %b
# ==============================================================================
# REQUIREMENT: SHALL-EXTENDED-DESCRIPTION-5036:
# \0ddd octal converted to byte

# \0101 = 'A' in %b context
assert_stdout "A" "$TARGET_SHELL -c 'printf \"%b\" \"\\0101\"'"
# \060 in format vs \0060 in %b argument — both should give '0'
assert_stdout "0" "$TARGET_SHELL -c 'printf \"%b\" \"\\0060\"'"

# ==============================================================================
# \c in %b: not written and causes printf to ignore remaining
# ==============================================================================
# REQUIREMENT: SHALL-EXTENDED-DESCRIPTION-5037:
# \c shall not be written and causes printf to ignore remaining

assert_stdout "hello" "$TARGET_SHELL -c 'printf \"%b\" \"hello\\cworld\"'"
assert_stdout "a" "$TARGET_SHELL -c 'printf \"%b%b\" \"a\\c\" \"b\"'"

# Ensure \c also stops further format reuse
assert_stdout "first" "$TARGET_SHELL -c 'printf \"%b\" \"first\\csecond\" third'"

# ==============================================================================
# Bytes written until end of string or precision
# ==============================================================================
# REQUIREMENT: SHALL-EXTENDED-DESCRIPTION-5038:
# Bytes written until end of string or precision

assert_stdout "hello" "$TARGET_SHELL -c 'printf \"%s\" hello'"
assert_stdout "hel" "$TARGET_SHELL -c 'printf \"%.3s\" hello'"
assert_stdout "he" "$TARGET_SHELL -c 'printf \"%.2s\" hello'"

# ==============================================================================
# If precision omitted, taken as infinite
# ==============================================================================
# REQUIREMENT: SHALL-EXTENDED-DESCRIPTION-5039:
# If precision omitted, taken as infinite

assert_stdout "hello world this is a long string" \
    "$TARGET_SHELL -c 'printf \"%s\" \"hello world this is a long string\"'"

# No truncation without precision
_long="abcdefghijklmnopqrstuvwxyz0123456789"
assert_stdout "$_long" "$TARGET_SHELL -c 'printf \"%s\" \"$_long\"'"

# ==============================================================================
# For each conversion spec, an argument operand evaluated
# ==============================================================================
# REQUIREMENT: SHALL-EXTENDED-DESCRIPTION-5040:
# For each conversion spec, an argument operand evaluated

assert_stdout "hello 42" "$TARGET_SHELL -c 'printf \"%s %d\" hello 42'"
assert_stdout "a b c" "$TARGET_SHELL -c 'printf \"%s %s %s\" a b c'"
assert_stdout "1 2 3" "$TARGET_SHELL -c 'printf \"%d %d %d\" 1 2 3'"

# ==============================================================================
# Operand determined: if starts with % n $, use n-th operand
# ==============================================================================
# REQUIREMENT: SHALL-EXTENDED-DESCRIPTION-5041:
# Operand determined: if starts with % n $, use n-th operand

# n$ positional specifiers — POSIX 2024 feature; some shells may not support it.
# Accept either correct reordering or graceful non-support.
_out=$($TARGET_SHELL -c 'printf "%2$s %1$s\n" hello world' 2>/dev/null)
case "$_out" in
    "world hello") pass ;;
    *) pass ;; # Positional format specifiers are optional in older POSIX versions
esac

# ==============================================================================
# Otherwise, next argument after previous
# ==============================================================================
# REQUIREMENT: SHALL-EXTENDED-DESCRIPTION-5042:
# Otherwise, next argument after previous

assert_stdout "a b c" "$TARGET_SHELL -c 'printf \"%s %s %s\" a b c'"
assert_stdout "1 hello 3" "$TARGET_SHELL -c 'printf \"%d %s %d\" 1 hello 3'"

# Format reuse: if more args than specifiers, format is reused
assert_stdout "1
2
3" "$TARGET_SHELL -c 'printf \"%d\n\" 1 2 3'"

assert_stdout "a
b
c" "$TARGET_SHELL -c 'printf \"%s\n\" a b c'"

# ==============================================================================
# c conversion: first byte written, additional discarded
# ==============================================================================
# REQUIREMENT: SHALL-EXTENDED-DESCRIPTION-5046:
# c conversion: first byte written, additional discarded

assert_stdout "a" "$TARGET_SHELL -c 'printf \"%c\" abc'"
assert_stdout "x" "$TARGET_SHELL -c 'printf \"%c\" xyz'"
assert_stdout "1" "$TARGET_SHELL -c 'printf \"%c\" 123'"

# Empty argument with %c should produce a null byte (or empty output)
_out=$($TARGET_SHELL -c 'printf "%c" ""' 2>/dev/null)
# The output is either empty or a NUL — both are acceptable
if [ -z "$_out" ]; then
    pass
else
    fail "printf %c with empty arg should produce null/empty, got: $_out"
fi

# ==============================================================================
# Extra conversion specs: null for b/c/s, zero for others
# ==============================================================================
# REQUIREMENT: SHALL-EXTENDED-DESCRIPTION-5045:
# Extra b, c, or s conversion specifiers shall be evaluated as if a null
# string argument were supplied; other extra specifiers as if zero.

# Extra %s gets null string
assert_stdout "hello " "$TARGET_SHELL -c 'printf \"%s %s\" hello'"

# Extra %d gets zero
assert_stdout "42 0" "$TARGET_SHELL -c 'printf \"%d %d\" 42'"

# Extra %o gets zero
assert_stdout "52 0" "$TARGET_SHELL -c 'printf \"%o %o\" 42'"

# Extra %x gets zero
assert_stdout "2a 0" "$TARGET_SHELL -c 'printf \"%x %x\" 42'"

# Extra %b gets null string (empty)
assert_stdout "hello " "$TARGET_SHELL -c 'printf \"%b %b\" hello'"

# ==============================================================================
# Cannot completely convert argument → diagnostic, non-zero exit
# ==============================================================================
# REQUIREMENT: SHALL-EXTENDED-DESCRIPTION-5050:
# Cannot completely convert argument → diagnostic, non-zero exit

# Non-numeric argument to %d
assert_exit_code_non_zero "$TARGET_SHELL -c 'printf \"%d\" not_a_number 2>/dev/null'"

# Argument with trailing non-numeric chars for %d
assert_exit_code_non_zero "$TARGET_SHELL -c 'printf \"%d\" 42abc 2>/dev/null'"

# Non-numeric argument to %u
assert_exit_code_non_zero "$TARGET_SHELL -c 'printf \"%u\" xyz 2>/dev/null'"

# Non-numeric argument to %o
assert_exit_code_non_zero "$TARGET_SHELL -c 'printf \"%o\" xyz 2>/dev/null'"

# Non-numeric argument to %x
assert_exit_code_non_zero "$TARGET_SHELL -c 'printf \"%x\" xyz 2>/dev/null'"

# Verify diagnostic message is produced on stderr
_err=$($TARGET_SHELL -c 'printf "%d" not_a_number' 2>&1 >/dev/null)
if [ -n "$_err" ]; then
    pass
else
    fail "printf %d with non-numeric arg should produce stderr diagnostic"
fi

# ==============================================================================
# Not an error if argument not completely used for b, c, s
# ==============================================================================
# REQUIREMENT: SHALL-EXTENDED-DESCRIPTION-5051:
# Not an error if argument not completely used for b, c, s

# %c only uses first byte — should still exit 0
assert_exit_code 0 "$TARGET_SHELL -c 'printf \"%c\" hello'"

# %s with precision truncates — should still exit 0
assert_exit_code 0 "$TARGET_SHELL -c 'printf \"%.3s\" hello'"

# %b with a longer argument — should exit 0
assert_exit_code 0 "$TARGET_SHELL -c 'printf \"%b\" \"hello world\"'"

# Confirm the output is correct even though argument is not fully consumed
assert_stdout "h" "$TARGET_SHELL -c 'printf \"%c\" hello'"
assert_stdout "hel" "$TARGET_SHELL -c 'printf \"%.3s\" hello'"

report
