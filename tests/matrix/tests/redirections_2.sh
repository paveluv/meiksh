# Test: Advanced Redirections
# Target: tests/matrix/tests/redirections_2.sh
#
# POSIX Shell redirections can handle duplicating file descriptors, closing
# them, read-write openings, and stripping tabs from here-documents. This
# suite thoroughly validates these advanced mechanics.

. "$MATRIX_DIR/lib.sh"

# ==============================================================================
# General Redirection Rules
# ==============================================================================
# REQUIREMENT: SHALL-2-7-189:
# The number n is an optional one or more digit decimal number designating the
# file descriptor number; the application shall ensure it is delimited from any
# preceding text and immediately precedes the redirection operator redir-op
# (with no intervening <blank> characters allowed).
# REQUIREMENT: SHALL-2-7-192:
# The largest file descriptor number supported in shell redirections is
# implementation-defined; however, all implementations shall support at least 0
# to 9, inclusive, for use by the application.
# REQUIREMENT: SHALL-2-7-193:
# If the redirection operator is "<<" or "<<-" , the word that follows the
# redirection operator shall be subjected to quote removal; it is unspecified
# whether any of the other expansions occur.
# REQUIREMENT: SHALL-2-7-191:
# The optional number, redirection operator, and word shall not appear in the
# arguments provided to the command to be executed (if any).

# We test that fd `3` works correctly and does not appear in arguments.
test_cmd='echo "fd3 content" 3>tmp_fd3.txt; cat tmp_fd3.txt'
assert_stdout "fd3 content" \
    "$TARGET_SHELL -c '$test_cmd'"
# REQUIREMENT: SHALL-2-7-194:
# For the other redirection operators, the word that follows the redirection
# operator shall be subjected to tilde expansion, parameter expansion, command
# substitution, arithmetic expansion, and quote removal.
# REQUIREMENT: SHALL-2-7-195:
# Pathname expansion shall not be performed on the word by a non-interactive
# shell; an interactive shell may perform it, but if the expansion would result
# in more than one word it is unspecified whether the redirection proceeds
# without pathname expansion being performed or the redirection fails.
# REQUIREMENT: SHALL-2-7-196:
# A failure to open or create a file shall cause a redirection to fail.

# A failed redirection (e.g. to a read-only directory) fails the command.
mkdir -p tmp_ro_dir
chmod -w tmp_ro_dir
test_cmd='echo "fail" > tmp_ro_dir/file.txt'
assert_exit_code_non_zero \
    "$TARGET_SHELL -c '$test_cmd'"

# Pathname expansion does not occur on the redirection word. It is treated
# literally as `*`.
test_cmd='echo "literal" > tmp_*_redir.txt; ls tmp_*_redir.txt'
assert_stdout "tmp_*_redir.txt" \
    "$TARGET_SHELL -c '$test_cmd'"

# Redirection words are subject to parameter expansion.
test_cmd='file_var="tmp_var_redir.txt"; echo "expanded" > "$file_var"; cat tmp_var_redir.txt'
assert_stdout "expanded" \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# Open Mechanics
# ==============================================================================
# REQUIREMENT: SHALL-2-7-1-197:
# Input redirection shall cause the file whose name results from the expansion
# of word to be opened for reading on the designated file descriptor, or
# standard input if the file descriptor is not specified.
# REQUIREMENT: SHALL-2-7-2-201:
# The check for existence, file creation, and open operations shall be
# performed atomically as is done by the open () function as defined in System
# Interfaces volume of POSIX.1-2024 when the O_CREAT and O_EXCL flags are set,
# except that if the file exists and is a symbolic link, the open operation need
# not fail with [EEXIST] unless the symbolic link resolves to an existing
# regular file.
# REQUIREMENT: SHALL-2-7-2-203:
# If the file does not exist, it shall be created as an empty file; otherwise,
# it shall be opened as if the open () function was called with the O_TRUNC flag
# set.
# REQUIREMENT: SHALL-2-7-3-205:
# The file shall be opened as if the open () function as defined in the System
# Interfaces volume of POSIX.1-2024 was called with the O_APPEND flag set.

test_cmd='echo "initial" > tmp_append.txt; echo "append" >> tmp_append.txt; cat tmp_append.txt'
assert_stdout "initial
append" \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# Here-Document Mechanics
# ==============================================================================
# REQUIREMENT: SHALL-2-7-4-207:
# The here-document shall be treated as a single word that begins after the
# next NEWLINE token and continues until there is a line containing only the
# delimiter and a <newline>, with no <blank> characters in between.
# REQUIREMENT: SHALL-2-7-4-208:
# For the purposes of locating this terminating line, the end of a
# command_string operand (see sh ) shall be treated as a <newline> character,
# and the end of the commands string in $( commands ) and ` commands ` may be
# treated as a <newline>.
# REQUIREMENT: SHALL-2-7-4-211:
# The removal of <backslash><newline> for line continuation (see 2.2.1 Escape
# Character (Backslash) ) shall be performed during the search for the trailing
# delimiter. (As a consequence, the trailing delimiter is not recognized
# immediately after a <newline> that was removed by line continuation.) It is
# unspecified whether the line containing the trailing delimiter is itself
# subject to this line continuation.
# REQUIREMENT: SHALL-2-7-4-212:
# All lines of the here-document shall be expanded, when the redirection
# operator is evaluated but after the trailing delimiter for the here-document
# has been located, for parameter expansion, command substitution, and
# arithmetic expansion.
# REQUIREMENT: SHALL-2-7-4-213:
# If the redirection operator is never evaluated (because the command it is
# part of is not executed), the here-document shall be read without performing
# any expansions.
# REQUIREMENT: SHALL-2-7-4-215:
# However, the double-quote character ( '"' ) shall not be treated specially
# within a here-document, except when the double-quote appears within "$()" ,
# "``" , or "${}" .
# REQUIREMENT: SHALL-2-7-4-219:
# When a here-document is read from a terminal device and the shell is
# interactive, it shall write the contents of the variable PS2 , processed as
# described in 2.5.3 Shell Variables , to standard error before reading each
# line of input until the delimiter has been recognized.

test_cmd='var="expanded"; cat <<EOF
this is $var
backslash \
continues
double "quotes"
EOF'
assert_stdout "this is expanded
backslash continues
double \"quotes\"" \
    "$TARGET_SHELL -c '$test_cmd'"

# A here-document isn't expanded if the command isn't executed.
test_cmd='if false; then cat <<EOF
this contains an invalid $var_that_fails_if_expanded
EOF
fi; echo "survived"'
assert_stdout "survived" \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# Multiple Here-Documents
# ==============================================================================
# REQUIREMENT: SHALL-2-7-4-218:
# If more than one "<<" or "<<-" operator is specified on a line, the
# here-document associated with the first operator shall be supplied first by
# the application and shall be read first by the shell.

test_cmd='cat <<EOF1; cat <<EOF2
first
EOF1
second
EOF2'
assert_stdout "first
second" \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# Stripping Tabs in Here-Documents (<<-)
# ==============================================================================
# REQUIREMENT: SHALL-2-7-4-216:
# If the redirection operator is "<<-" , all leading <tab> characters shall be
# stripped from input lines after <backslash><newline> line continuation (when
# it applies) has been performed, and from the line containing the trailing
# delimiter.
# REQUIREMENT: SHALL-2-7-4-217:
# Stripping of leading <tab> characters shall occur as the here-document is
# read from the shell input (and consequently does not affect any <tab>
# characters that result from expansions).

# We use an actual tab character here.
test_cmd='cat <<-EOF
	line 1
		line 2
EOF'
assert_stdout "line 1
line 2" \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# Read-Write Redirections (<>)
# ==============================================================================
# REQUIREMENT: SHALL-2-7-7-227:
# The redirection operator: [ n ] <> word shall cause the file whose name is
# the expansion of word to be opened for both reading and writing on the file
# descriptor denoted by n , or standard input if n is not specified.
# REQUIREMENT: SHALL-2-7-3-206:
# If the file does not exist, it shall be created.

# The `tmp_rw.txt` doesn't exist, so `<>` creates it. Then we echo into fd 3.
test_cmd='echo "rw test" 3<>tmp_rw.txt 1>&3; cat tmp_rw.txt'
assert_stdout "rw test" \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# Duplicating and Closing Output File Descriptors (>&)
# ==============================================================================
# REQUIREMENT: SHALL-2-7-6-224:
# The redirection operator: [ n ] >& word shall duplicate one output file
# descriptor from another, or shall close one.
# REQUIREMENT: SHALL-2-7-6-225:
# If word evaluates to one or more digits, the file descriptor denoted by n ,
# or standard output if n is not specified, shall be made to be a copy of the
# file descriptor denoted by word ; if the digits in word do not represent an
# already open file descriptor, a redirection error shall result (see 2.8.1
# Consequences of Shell Errors ); if the file descriptor denoted by word
# represents an open file descriptor that is not open for output, a redirection
# error may result.
# REQUIREMENT: SHALL-2-7-5-223:
# Attempts to close a file descriptor that is not open shall not constitute an
# error.

# We redirect stdout (1) to fd 4, then echo to stdout, which lands in
# tmp_dup.txt
test_cmd='exec 4>tmp_dup.txt; echo "dup test" >&4; exec 4>&-; cat tmp_dup.txt'
assert_stdout "dup test" \
    "$TARGET_SHELL -c '$test_cmd'"

# Closing an unopened file descriptor (like fd 9) should not fail the command.
test_cmd='echo "ok" 9>&-'
assert_stdout "ok" \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# Duplicating and Closing Input File Descriptors (<&)
# ==============================================================================
# REQUIREMENT: SHALL-2-7-5-220:
# The redirection operator: [ n ] <& word shall duplicate one input file
# descriptor from another, or shall close one.
# REQUIREMENT: SHALL-2-7-5-221:
# If word evaluates to one or more digits, the file descriptor denoted by n ,
# or standard input if n is not specified, shall be made to be a copy of the
# file descriptor denoted by word ; if the digits in word do not represent an
# already open file descriptor, a redirection error shall result (see 2.8.1
# Consequences of Shell Errors ); if the file descriptor denoted by word
# represents an open file descriptor that is not open for input, a redirection
# error may result.
# REQUIREMENT: SHALL-2-7-5-222:
# If word evaluates to '-' , file descriptor n , or standard input if n is not
# specified, shall be closed.
# REQUIREMENT: SHALL-2-7-5-223:
# Attempts to close a file descriptor that is not open shall not constitute an
# error.

echo "input dup" > tmp_in.txt
test_cmd='exec 5<tmp_in.txt; cat <&5; exec 5<&-'
assert_stdout "input dup" \
    "$TARGET_SHELL -c '$test_cmd'"

# Closing an unopened fd for input.
test_cmd='echo "ok" 8<&-'
assert_stdout "ok" \
    "$TARGET_SHELL -c '$test_cmd'"


report
