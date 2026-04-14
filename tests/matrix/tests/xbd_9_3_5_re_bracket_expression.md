# Test Suite for XBD 9.3.5 RE Bracket Expression

This test suite covers XBD Section 9.3.5 (RE Bracket Expression) from the
POSIX.1-2024 Base Definitions. These rules are relevant to the shell because
the shell's pattern matching notation (Section 2.14) uses bracket expressions
that follow the same semantics defined here.

## Table of contents

- [xbd: 9.3.5 RE Bracket Expression](#xbd-935-re-bracket-expression)

## xbd: 9.3.5 RE Bracket Expression

A bracket expression (an expression enclosed in square brackets, `"[]"`) is an RE that shall match a specific set of single characters, and may match a specific set of multi-character collating elements, based on the non-empty set of list expressions contained in the bracket expression.

The following rules and definitions apply to bracket expressions:

1. A bracket expression is either a matching list expression or a non-matching list expression. It consists of one or more expressions: ordinary characters, collating elements, collating symbols, equivalence classes, character classes, or range expressions. The `<right-square-bracket>` (`']'`) shall lose its special meaning and represent itself in a bracket expression if it occurs first in the list (after an initial `<circumflex>` (`'^'`), if any). Otherwise, it shall terminate the bracket expression, unless it appears in a collating symbol (such as `"[.].]"`) or is the ending `<right-square-bracket>` for a collating symbol, equivalence class, or character class. When the bracket expression appears within a BRE, the special characters `'.'`, `'*'`, `'['`, and `'\\'` (`<period>`, `<asterisk>`, `<left-square-bracket>`, and `<backslash>`, respectively) shall lose their special meaning within the bracket expression. When the bracket expression appears within an ERE, the special characters `'.'`, `'('`, `'*'`, `'+'`, `'?'`, `'{'`, `'|'`, `'$'`, `'['`, and `'\\'` (`<period>`, `<left-parenthesis>`, `<asterisk>`, `<plus-sign>`, `<question-mark>`, `<left-brace>`, `<vertical-line>`, `<dollar-sign>`, `<left-square-bracket>`, and `<backslash>`, respectively) shall lose their special meaning within the bracket expression; `<circumflex>` (`'^'`) shall lose its special meaning as an anchor. When the bracket expression appears within a shell pattern (see XCU [*2.14 Pattern Matching Notation*](docs/posix/md/utilities/V3_chap02.md#214-pattern-matching-notation)), the special characters `'?'`, `'*'`, and `'['` (`<question-mark>`, `<asterisk>`, and `<left-square-bracket>`, respectively) shall lose their special meaning within the bracket expression; whether or not `<backslash>` (`'\\'`) loses its special meaning as a pattern matching character is described in XCU [*2.14.1 Patterns Matching a Single Character*](docs/posix/md/utilities/V3_chap02.md#2141-patterns-matching-a-single-character), but in contexts where a shell-quoting `<backslash>` can be used it shall retain its special meaning (see XCU [*2.2 Quoting*](docs/posix/md/utilities/V3_chap02.md#22-quoting)). For example: The character sequences `"[."`, `"[="`, and `"[:"` (`<left-square-bracket>` followed by a `<period>`, `<equals-sign>`, or `<colon>`) shall be special inside a bracket expression and are used to delimit collating symbols, equivalence class expressions, and character class expressions. These symbols shall be followed by a valid expression and the matching terminating sequence `".]"`, `"=]"`, or `":]"`, as described in the following items.
  ```
  $ ls
  ! $ - \ a b c
  $ echo [a\-c]
  - a c
  $ echo [\!a]
  ! a
  $ echo ["!\$a-c"]
  ! $ - a c
  $ echo [!"\$a-c"]
  ! \ b
  $ echo [!\]\\]
  ! $ - a b c
  ```
2. A matching list expression specifies a list that shall match any single character that is matched by one of the expressions represented in the list. The first character in the list cannot be the `<circumflex>`. An ordinary character in the list shall only match that character; for example, `"[abc]"` is an RE that only matches one of the characters `'a'`, `'b'`, or `'c'`. It is unspecified whether a matching list expression matches a multi-character collating element that is matched by one of the expressions.
3. A non-matching list expression begins with a `<circumflex>` (`'^'`), and the matching behavior shall be the logical inverse of the corresponding matching list expression (the same bracket expression but without the leading `<circumflex>`). For example, since the RE `"[abc]"` only matches `'a'`, `'b'`, or `'c'`, it follows that `"[^abc]"` is an RE that matches any character except `'a'`, `'b'`, or `'c'`. It is unspecified whether a non-matching list expression matches a multi-character collating element that is not matched by any of the expressions. The `<circumflex>` shall have this special meaning only when it occurs first in the list, immediately following the `<left-square-bracket>`.
4. A collating symbol is a collating element enclosed within bracket-period (`"[."` and `".]"`) delimiters. Collating elements are defined as described in [*7.3.2.4 Collation Order*](docs/posix/md/basedefs/V1_chap07.md#7324-collation-order). Conforming applications shall represent multi-character collating elements as collating symbols when it is necessary to distinguish them from a list of the individual characters that make up the multi-character collating element. For example, if the string `"ch"` is a collating element defined using the line: in the locale definition, the expression `"[[.ch.]]"` shall be treated as an RE containing the collating symbol `'ch'`, while `"[ch]"` shall be treated as an RE matching `'c'` or `'h'`. Collating symbols are recognized only inside bracket expressions. If the string is not a collating element in the current locale, the expression is invalid.
  ```
  collating-element <ch-digraph> from "<c><h>"
  ```
5. An equivalence class expression shall represent the set of collating elements belonging to an equivalence class, as described in [*7.3.2.4 Collation Order*](docs/posix/md/basedefs/V1_chap07.md#7324-collation-order). Only primary equivalence classes shall be recognized. The class shall be expressed by enclosing any one of the collating elements in the equivalence class within bracket-equal (`"[="` and `"=]"`) delimiters. For example, if `'a'`, `'à'`, and `'â'` belong to the same equivalence class, then `"[[=a=]b]"`, `"[[=à=]b]"`, and `"[[=â=]b]"` are each equivalent to `"[aàâb]"`. If the collating element does not belong to an equivalence class, the equivalence class expression shall be treated as a collating symbol.
6. A character class expression shall represent the union of two sets: All character classes specified in the current locale shall be recognized. A character class expression is expressed as a character class name enclosed within bracket-`<colon>` (`"[:"` and `":]"`) delimiters. The following character class expressions shall be supported in all locales: In addition, character class expressions of the form: are recognized in those locales where the *name* keyword has been given a **charclass** definition in the *LC_CTYPE* category.
    1. The set of single characters that belong to the character class, as defined in the *LC_CTYPE* category in the current locale.
    2. An unspecified set of multi-character collating elements.
  ```
  [:alnum:]   [:cntrl:]   [:lower:]   [:space:]
  [:alpha:]   [:digit:]   [:print:]   [:upper:]
  [:blank:]   [:graph:]   [:punct:]   [:xdigit:]
  ```
  ```
  [:name:]
  ```
7. In the POSIX locale, a range expression represents the set of collating elements that fall between two elements in the collation sequence, inclusive. In other locales, a range expression has unspecified behavior: strictly conforming applications shall not rely on whether the range expression is valid, or on the set of collating elements matched. A range expression shall be expressed as the starting point and the ending point separated by a `<hyphen-minus>` (`'-'`). In the following, all examples assume the POSIX locale. The starting range point and the ending range point shall be a collating element or collating symbol. An equivalence class expression used as a starting or ending point of a range expression produces unspecified results. An equivalence class can be used portably within a bracket expression, but only outside the range. If the represented set of collating elements is empty, it is unspecified whether the expression matches nothing, or is treated as invalid. The interpretation of range expressions where the ending range point is also the starting range point of a subsequent range expression (for example, `"[a-m-o]"`) is undefined. The `<hyphen-minus>` character shall be treated as itself if it occurs first (after an initial `'^'`, if any) or last in the list, or as an ending range point in a range expression. As examples, the expressions `"[-ac]"` and `"[ac-]"` are equivalent and match any of the characters `'a'`, `'c'`, or `'-'`; `"[^-ac]"` and `"[^ac-]"` are equivalent and match any characters except `'a'`, `'c'`, or `'-'`; the expression `"[%--]"` matches any of the characters between `'%'` and `'-'` inclusive; the expression `"[--@]"` matches any of the characters between `'-'` and `'@'` inclusive; and the expression `"[a--@]"` is either invalid or equivalent to `'@'`, because the letter `'a'` follows the symbol `'-'` in the POSIX locale. To use a `<hyphen-minus>` as the starting range point, it shall either come first in the bracket expression or be specified as a collating symbol; for example, `"[][.-.]-0]"`, which matches either a `<right-square-bracket>` or any character or collating element that collates between `<hyphen-minus>` and 0, inclusive. If a bracket expression specifies both `'-'` and `']'`, the `']'` shall be placed first (after the `'^'`, if any) and the `'-'` last within the bracket expression.
8. If a bracket expression contains at least three list elements, where the first and last list elements are the same single-character element of `<period>`, `<equals-sign>`, or `<colon>`, then it is unspecified whether the bracket expression will be treated as a collating symbol, equivalence class, or character class, respectively; treated as a matching list expression; or treated as an invalid bracket expression.

### Tests

#### Test: circumflex negation in bracket expressions

A non-matching list expression begins with `!` (or `^` in regex context).
The matching behavior is the logical inverse of the corresponding
matching list, and `!` is literal when it does not appear first.

```
begin test "circumflex negation in bracket expressions"
  script
    case b in [!ac]) echo "match b" ;; *) echo "no match b" ;; esac
    case a in [!ac]) echo "match a" ;; *) echo "no match a" ;; esac
    case a in [a!c]) echo "match a" ;; *) echo "no match a" ;; esac
    case '!' in [a!c]) echo "match !" ;; *) echo "no match !" ;; esac
  expect
    stdout "match b\nno match a\nmatch a\nmatch !"
    stderr ""
    exit_code 0
end test "circumflex negation in bracket expressions"
```

#### Test: right-bracket first in bracket expression

The `]` character placed first in a bracket expression (after initial `^` if any) is treated as a literal character to match.

```
begin test "right-bracket first in bracket expression"
  setenv "LC_ALL" "test_EPTY.UTF-8"
  script
    case "]" in
      []abc]) echo "match";;
      *) echo "nomatch";;
    esac
  expect
    stdout "match"
    stderr ""
    exit_code 0
end test "right-bracket first in bracket expression"
```

#### Test: right-bracket terminates bracket expression

When `]` is not the first character in a bracket expression, it terminates the expression. Characters not in the list are not matched.

```
begin test "right-bracket terminates bracket expression"
  setenv "LC_ALL" "test_EPTY.UTF-8"
  script
    case "b" in
      [ab]) echo "match";;
      *) echo "nomatch";;
    esac
    case "]" in
      [ab]) echo "bracket_match";;
      *) echo "bracket_nomatch";;
    esac
  expect
    stdout "match\nbracket_nomatch"
    stderr ""
    exit_code 0
end test "right-bracket terminates bracket expression"
```

#### Test: special BRE chars lose meaning in bracket expression

When the bracket expression appears within a BRE, the special characters `.`, `*`, `[`, and `\` shall lose their special meaning within the bracket expression.

```
begin test "special BRE chars lose meaning in bracket expression"
  setenv "LC_ALL" "test_EPTY.UTF-8"
  script
    echo '.' | grep '[.*]' >/dev/null && echo "dot_match"
    echo '*' | grep '[.*]' >/dev/null && echo "star_match"
    echo 'x' | grep '[.*]' >/dev/null && echo "x_match" || echo "x_nomatch"
  expect
    stdout "dot_match\nstar_match\nx_nomatch"
    stderr ""
    exit_code 0
end test "special BRE chars lose meaning in bracket expression"
```

#### Test: special ERE chars lose meaning in bracket expression

When the bracket expression appears within an ERE, the special
characters `.`, `(`, `*`, `+`, `?`, `{`, `|`, `$`, `[`, and `\` shall
lose their special meaning within the bracket expression, and `^` shall
lose its anchor meaning.

```
begin test "special ERE chars lose meaning in bracket expression"
  setenv "LC_ALL" "test_EPTY.UTF-8"
  script
    echo '(' | grep -E '[(]' >/dev/null && echo "paren_match"
    echo '+' | grep -E '[+]' >/dev/null && echo "plus_match"
    echo '^' | grep -E '[a^]' >/dev/null && echo "caret_match"
    echo '$' | grep -E '[$]' >/dev/null && echo "dollar_match"
    echo 'x' | grep -E '[(+^$]' >/dev/null && echo "x_match" || echo "x_nomatch"
  expect
    stdout "paren_match\nplus_match\ncaret_match\ndollar_match\nx_nomatch"
    stderr ""
    exit_code 0
end test "special ERE chars lose meaning in bracket expression"
```

#### Test: special shell pattern chars lose meaning in bracket expression

Within a shell pattern bracket expression, `?`, `*`, and `[` lose their special meaning as glob metacharacters.

```
begin test "special shell pattern chars lose meaning in bracket expression"
  setenv "LC_ALL" "test_EPTY.UTF-8"
  script
    case '?' in
      [?*]) echo "qmark_match";;
      *) echo "qmark_nomatch";;
    esac
    case '*' in
      [?*]) echo "star_match";;
      *) echo "star_nomatch";;
    esac
    case 'x' in
      [?*]) echo "x_match";;
      *) echo "x_nomatch";;
    esac
  expect
    stdout "qmark_match\nstar_match\nx_nomatch"
    stderr ""
    exit_code 0
end test "special shell pattern chars lose meaning in bracket expression"
```

#### Test: matching list expression

A matching list expression matches any single character that is matched by one of the expressions in the list. Combinations of character classes and ranges are supported.

```
begin test "matching list expression"
  setenv "LC_ALL" "test_EPTY.UTF-8"
  script
    case "3" in
      [[:digit:]a-f]) echo "match";;
      *) echo "nomatch";;
    esac
    case "e" in
      [[:digit:]a-f]) echo "match2";;
      *) echo "nomatch2";;
    esac
    case "z" in
      [[:digit:]a-f]) echo "match3";;
      *) echo "nomatch3";;
    esac
  expect
    stdout "match\nmatch2\nnomatch3"
    stderr ""
    exit_code 0
end test "matching list expression"
```

#### Test: ordinary character in bracket expression

An ordinary character in the list shall only match that character. `[abc]` matches `a`, `b`, or `c` but not `d`.

```
begin test "ordinary character in bracket expression"
  setenv "LC_ALL" "test_EPTY.UTF-8"
  script
    case "a" in
      [abc]) echo "match";;
      *) echo "nomatch";;
    esac
    case "d" in
      [abc]) echo "match2";;
      *) echo "nomatch2";;
    esac
  expect
    stdout "match\nnomatch2"
    stderr ""
    exit_code 0
end test "ordinary character in bracket expression"
```

#### Test: circumflex only special when first

The circumflex has its special negation meaning only when it occurs first in the list. In other positions it is treated as a literal character.

```
begin test "circumflex only special when first"
  setenv "LC_ALL" "test_EPTY.UTF-8"
  script
    case "^" in
      [a^b]) echo "match";;
      *) echo "nomatch";;
    esac
  expect
    stdout "match"
    stderr ""
    exit_code 0
end test "circumflex only special when first"
```

#### Test: collating symbol basic syntax

A collating symbol is a collating element enclosed within bracket-period (`[.` and `.]`) delimiters. `[[.a.]]` matches the character `a`.

```
begin test "collating symbol basic syntax"
  setenv "LC_ALL" "test_EPTY.UTF-8"
  script
    case "a" in
      [[.a.]]) echo "match";;
      *) echo "nomatch";;
    esac
  expect
    stdout "match"
    stderr ""
    exit_code 0
end test "collating symbol basic syntax"
```

#### Test: equivalence class expression syntax

An equivalence class expression represents the set of collating elements belonging to an equivalence class, enclosed within bracket-equal (`[=` and `=]`) delimiters.

```
begin test "equivalence class expression syntax"
  setenv "LC_ALL" "test_EPTY.UTF-8"
  script
    case "a" in
      [[=a=]]) echo "match";;
      *) echo "nomatch";;
    esac
  expect
    stdout "match"
    stderr ""
    exit_code 0
end test "equivalence class expression syntax"
```

#### Test: primary equivalence class recognition

Only primary equivalence classes shall be recognized. `[[=a=]]` matches `a` and any characters in its equivalence class.

```
begin test "primary equivalence class recognition"
  setenv "LC_ALL" "test_EPTY.UTF-8"
  script
    case "a" in
      [[=a=]]) echo "match";;
      *) echo "nomatch";;
    esac
  expect
    stdout "match"
    stderr ""
    exit_code 0
end test "primary equivalence class recognition"
```

#### Test: equivalence class bracket notation

An equivalence class can be combined with other elements in a bracket expression. `[[=a=]bc]` matches `a`, `b`, or `c`.

```
begin test "equivalence class bracket notation"
  setenv "LC_ALL" "test_EPTY.UTF-8"
  script
    case "a" in
      [[=a=]bc]) echo "match";;
      *) echo "nomatch";;
    esac
    case "b" in
      [[=a=]bc]) echo "match2";;
      *) echo "nomatch2";;
    esac
  expect
    stdout "match\nmatch2"
    stderr ""
    exit_code 0
end test "equivalence class bracket notation"
```

#### Test: equivalence class fallback to collating symbol

If the collating element does not belong to an equivalence class, the equivalence class expression shall be treated as a collating symbol.

```
begin test "equivalence class fallback to collating symbol"
  setenv "LC_ALL" "test_EPTY.UTF-8"
  script
    case "z" in
      [[=z=]]) echo "match";;
      *) echo "nomatch";;
    esac
  expect
    stdout "match"
    stderr ""
    exit_code 0
end test "equivalence class fallback to collating symbol"
```

#### Test: all character classes recognized in locale

The standard character class expressions `alnum`, `alpha`, `blank`,
`cntrl`, `digit`, `graph`, `lower`, `print`, `punct`, `space`, `upper`,
and `xdigit` shall be recognized.

```
begin test "all character classes recognized in locale"
  setenv "LC_ALL" "test_EPTY.UTF-8"
  script
    case "a" in [[:alnum:]]) echo "alnum_ok";; *) echo "alnum_fail";; esac
    case "a" in [[:alpha:]]) echo "alpha_ok";; *) echo "alpha_fail";; esac
    case " " in [[:blank:]]) echo "blank_ok";; *) echo "blank_fail";; esac
    case "$(printf '\001')" in [[:cntrl:]]) echo "cntrl_ok";; *) echo "cntrl_fail";; esac
    case "0" in [[:digit:]]) echo "digit_ok";; *) echo "digit_fail";; esac
    case "!" in [[:graph:]]) echo "graph_ok";; *) echo "graph_fail";; esac
    case "a" in [[:lower:]]) echo "lower_ok";; *) echo "lower_fail";; esac
    case "a" in [[:print:]]) echo "print_ok";; *) echo "print_fail";; esac
    case "!" in [[:punct:]]) echo "punct_ok";; *) echo "punct_fail";; esac
    case " " in [[:space:]]) echo "space_ok";; *) echo "space_fail";; esac
    case "A" in [[:upper:]]) echo "upper_ok";; *) echo "upper_fail";; esac
    case "F" in [[:xdigit:]]) echo "xdigit_ok";; *) echo "xdigit_fail";; esac
  expect
    stdout "alnum_ok\nalpha_ok\nblank_ok\ncntrl_ok\ndigit_ok\ngraph_ok\nlower_ok\nprint_ok\npunct_ok\nspace_ok\nupper_ok\nxdigit_ok"
    stderr ""
    exit_code 0
end test "all character classes recognized in locale"
```

#### Test: range expression in POSIX locale

In the POSIX locale, a range expression like `[a-z]` matches lowercase letters only. Uppercase `B` shall not match `[a-z]` in the POSIX locale.

```
begin test "range expression in POSIX locale"
  setenv "LC_ALL" "C"
  script
    case "b" in
      [a-z]) echo "match";;
      *) echo "nomatch";;
    esac
    case "B" in
      [a-z]) echo "upper_match";;
      *) echo "upper_nomatch";;
    esac
  expect
    stdout "match\nupper_nomatch"
    stderr ""
    exit_code 0
end test "range expression in POSIX locale"
```

#### Test: POSIX locale punctuation ranges

In the POSIX locale, ranges such as `[%--]` and `[--@]` shall match the
collating elements between the endpoints, inclusive.

```
begin test "POSIX locale punctuation ranges"
  setenv "LC_ALL" "C"
  script
    case "%" in [%--]) echo "percent_match";; *) echo "percent_nomatch";; esac
    case "+" in [%--]) echo "plus_match";; *) echo "plus_nomatch";; esac
    case "-" in [%--]) echo "hyphen_match";; *) echo "hyphen_nomatch";; esac
    case "-" in [--@]) echo "dash_match";; *) echo "dash_nomatch";; esac
    case "@" in [--@]) echo "at_match";; *) echo "at_nomatch";; esac
  expect
    stdout "percent_match\nplus_match\nhyphen_match\ndash_match\nat_match"
    stderr ""
    exit_code 0
end test "POSIX locale punctuation ranges"
```

#### Test: range expression basic syntax

A range expression is expressed as a starting point and ending point separated by a hyphen-minus. `[a-c]` matches `b` but not `d`.

```
begin test "range expression basic syntax"
  setenv "LC_ALL" "test_EPTY.UTF-8"
  script
    case "b" in
      [a-c]) echo "match";;
      *) echo "nomatch";;
    esac
    case "d" in
      [a-c]) echo "match2";;
      *) echo "nomatch2";;
    esac
  expect
    stdout "match\nnomatch2"
    stderr ""
    exit_code 0
end test "range expression basic syntax"
```

#### Test: hyphen literal when first or last

The hyphen-minus is treated as itself when it occurs first or last in a bracket expression list. `[-abc]` and `[abc-]` both match a literal `-`.

```
begin test "hyphen literal when first or last"
  setenv "LC_ALL" "test_EPTY.UTF-8"
  script
    case "-" in
      [-abc]) echo "first_match";;
      *) echo "first_nomatch";;
    esac
    case "-" in
      [abc-]) echo "last_match";;
      *) echo "last_nomatch";;
    esac
  expect
    stdout "first_match\nlast_match"
    stderr ""
    exit_code 0
end test "hyphen literal when first or last"
```

#### Test: hyphen as starting range point

To use a hyphen-minus as the starting range point of a range expression,
it shall come first in the bracket expression or be specified as a
collating symbol.

```
begin test "hyphen as starting range point"
  setenv "LC_ALL" "C"
  script
    echo '-' | grep '[][.-.]-0]' >/dev/null && echo "hyphen_match"
    echo '/' | grep '[][.-.]-0]' >/dev/null && echo "slash_match"
  expect
    stdout "hyphen_match\nslash_match"
    stderr ""
    exit_code 0
end test "hyphen as starting range point"
```

#### Test: bracket with both ] and -

When both `]` and `-` appear in a bracket expression, `]` shall be placed first and `-` last. `[]a-]` matches `]`, `a`, and `-` but not `b`.

```
begin test "bracket with both ] and -"
  setenv "LC_ALL" "test_EPTY.UTF-8"
  script
    case "]" in
      []a-]) echo "bracket_match";;
      *) echo "bracket_nomatch";;
    esac
    case "-" in
      []a-]) echo "hyphen_match";;
      *) echo "hyphen_nomatch";;
    esac
    case "a" in
      []a-]) echo "a_match";;
      *) echo "a_nomatch";;
    esac
    case "b" in
      []a-]) echo "b_match";;
      *) echo "b_nomatch";;
    esac
  expect
    stdout "bracket_match\nhyphen_match\na_match\nb_nomatch"
    stderr ""
    exit_code 0
end test "bracket with both ] and -"
```
