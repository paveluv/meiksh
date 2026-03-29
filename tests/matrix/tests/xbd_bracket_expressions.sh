#!/bin/sh

# Test: XBD 9.3.5 RE Bracket Expressions
# Target: tests/matrix/tests/xbd_bracket_expressions.sh
#
# This script tests the behavior of bracket expressions as defined in POSIX
# Base Definitions (XBD) 9.3.5. Specifically, we focus on pattern matching
# in the shell, verifying how bracket expressions are parsed and matched.

. "$MATRIX_DIR/lib.sh"

# REQUIREMENT: SHALL-XBD-9-3-5-2000:
# A bracket expression (an expression enclosed in square brackets, "[]" ) is an
# RE that shall match a specific set of single characters, and may match a
# specific set of multi-character collating elements, based on the non-empty set
# of list expressions contained in the bracket expression.
# REQUIREMENT: SHALL-XBD-9-3-5-2008:
# A matching list expression specifies a list that shall match any single
# character that is matched by one of the expressions represented in the list.
# REQUIREMENT: SHALL-XBD-9-3-5-2009:
# An ordinary character in the list shall only match that character; for
# example, "[abc]" is an RE that only matches one of the characters 'a' , 'b' ,
# or 'c'.
test_cmd='
    case b in
        [abc]) echo "match b" ;;
        *) echo "no match" ;;
    esac
    case z in
        [abc]) echo "match z" ;;
        *) echo "no match" ;;
    esac
'
assert_stdout "match b
no match" "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-XBD-9-3-5-2010:
# A non-matching list expression begins with a <circumflex> ( '^' ), and the
# matching behavior shall be the logical inverse of the corresponding matching
# list expression (the same bracket expression but without the leading
# <circumflex>).
# REQUIREMENT: SHALL-XBD-9-3-5-2011:
# The <circumflex> shall have this special meaning only when it occurs first in
# the list, immediately following the <left-square-bracket>.
test_cmd='
    case b in
        [!ac]) echo "match b" ;;
        *) echo "no match b" ;;
    esac
    case a in
        [!ac]) echo "match a" ;;
        *) echo "no match a" ;;
    esac
    case a in
        [a!c]) echo "match a" ;;
        *) echo "no match a" ;;
    esac
    case ! in
        [a!c]) echo "match !" ;;
        *) echo "no match !" ;;
    esac
'
assert_stdout "match b
no match a
match a
match !" "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-XBD-9-3-5-2001:
# The <right-square-bracket> ( ']' ) shall lose its special meaning and
# represent itself in a bracket expression if it occurs first in the list (after
# an initial <circumflex> ( '^' ), if any).
# REQUIREMENT: SHALL-XBD-9-3-5-2002:
# Otherwise, it shall terminate the bracket expression, unless it appears in a
# collating symbol (such as "[.].]" ) or is the ending <right-square-bracket>
# for a collating symbol, equivalence class, or character class.
test_cmd='
    case "]" in
        []a]) echo "match ]" ;;
        *) echo "no match ]" ;;
    esac
    case a in
        []a]) echo "match a" ;;
        *) echo "no match a" ;;
    esac
    case "]" in
        [!]]) echo "no match ]" ;;
        *) echo "match ]" ;;
    esac
    case a in
        [!]]) echo "match a" ;;
        *) echo "no match a" ;;
    esac
'
assert_stdout "match ]
match a
match ]
match a" "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-XBD-9-3-5-2005:
# When the bracket expression appears within a shell pattern (see XCU 2.14
# Pattern Matching Notation ), the special characters '?' , '*' , and '['
# (<question-mark>, <asterisk>, and <left-square-bracket>, respectively) shall
# lose their special meaning within the bracket expression; whether or not
# <backslash> ( '\\' ) loses its special meaning as a pattern matching character
# is described in XCU 2.14.1 Patterns Matching a Single Character , but in
# contexts where a shell-quoting <backslash> can be used it shall retain its
# special meaning (see XCU 2.2 Quoting ).
test_cmd='
    case "?" in
        [?]) echo "match ?" ;;
        *) echo "no match ?" ;;
    esac
    case "*" in
        [*]) echo "match *" ;;
        *) echo "no match *" ;;
    esac
    case "[" in
        [\[]) echo "match [" ;;
        *) echo "no match [" ;;
    esac
    case "\\" in
        [\\]) echo "match \\" ;;
        *) echo "no match \\" ;;
    esac
'
# In sh patterns, backslash inside bracket expression is usually literal or
# escapes?
# Wait, POSIX says "\ inside bracket expression retains its special meaning if
# quoting can be used".
# So [\\] matches \ because the first \ escapes the second.
# What about [\[]? The first \ escapes the [. It matches [.
assert_stdout "match ?
match *
match [
match \\" "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-XBD-9-3-5-2019:
# All character classes specified in the current locale shall be recognized.
# REQUIREMENT: SHALL-XBD-9-3-5-2006:
# The following character class expressions
# shall be supported in all locales: [:alnum:] [:cntrl:] [:lower:] [:space:]
# [:alpha:] [:digit:] [:print:] [:upper:] [:blank:] [:graph:] [:punct:]
# [:xdigit:]
# REQUIREMENT: SHALL-XBD-9-3-5-2002:
# A character class expression shall
# represent the union of two sets:
test_cmd='
    case "A" in
        [[:alpha:]]) echo "match A alpha" ;;
        *) echo "no match A alpha" ;;
    esac
    case "5" in
        [[:digit:]]) echo "match 5 digit" ;;
        *) echo "no match 5 digit" ;;
    esac
    case " " in
        [[:space:]]) echo "match space" ;;
        *) echo "no match space" ;;
    esac
    case "!" in
        [[:punct:]]) echo "match punct" ;;
        *) echo "no match punct" ;;
    esac
'
assert_stdout "match A alpha
match 5 digit
match space
match punct" "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-XBD-9-3-5-2022:
# A range expression shall be expressed as the starting point and the ending
# point separated by a <hyphen-minus> ( '-' ).
# REQUIREMENT: SHALL-XBD-9-3-5-2023:
# The starting range point and the ending range point shall be a collating
# element or collating symbol.
# REQUIREMENT: SHALL-XBD-9-3-5-2024:
# The <hyphen-minus> character shall be treated as itself if it occurs first
# (after an initial '^' , if any) or last in the list, or as an ending range
# point in a range expression.
# REQUIREMENT: SHALL-XBD-9-3-5-2025:
# To use a <hyphen-minus> as the starting range point, it shall either come
# first in the bracket expression or be specified as a collating symbol; for
# example, "[][.-.]-0]" , which matches either a <right-square-bracket> or any
# character or collating element that collates between <hyphen-minus> and 0,
# inclusive.
# REQUIREMENT: SHALL-XBD-9-3-5-2026:
# If a bracket expression specifies both '-' and ']' , the ']' shall be placed
# first (after the '^' , if any) and the '-' last within the bracket expression.
test_cmd='
    case b in
        [a-c]) echo "match b range" ;;
        *) echo "no match b" ;;
    esac
    case "-" in
        [-ac]) echo "match - first" ;;
        *) echo "no match" ;;
    esac
    case "-" in
        [ac-]) echo "match - last" ;;
        *) echo "no match" ;;
    esac
    case "-" in
        []ac-]) echo "match ] and -" ;;
        *) echo "no match" ;;
    esac
    case "]" in
        []ac-]) echo "match ] in ]ac-" ;;
        *) echo "no match" ;;
    esac
'
assert_stdout "match b range
match - first
match - last
match ] and -
match ] in ]ac-" "$TARGET_SHELL -c '$test_cmd'"

report
