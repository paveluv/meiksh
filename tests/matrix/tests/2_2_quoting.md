# Test Suite for 2.2 Quoting

This test suite covers **Section 2.2 Quoting** of the POSIX Shell Command Language
(POSIX.1-2024), which defines how the shell interprets special characters and the
four quoting mechanisms: the escape character (backslash), single-quotes,
double-quotes, and dollar-single-quotes.

## Table of contents

- [2.2 Quoting](#22-quoting)
- [2.2.1 Escape Character (Backslash)](#221-escape-character-backslash)
- [2.2.2 Single-Quotes](#222-single-quotes)
- [2.2.3 Double-Quotes](#223-double-quotes)
- [2.2.4 Dollar-Single-Quotes](#224-dollar-single-quotes)

## 2.2 Quoting

Quoting is used to remove the special meaning of certain characters or words to the shell. Quoting can be used to preserve the literal meaning of the special characters in the next paragraph, prevent reserved words from being recognized as such, and prevent parameter expansion and command substitution within here-document processing (see [2.7.4 Here-Document](#274-here-document) ).

The application shall quote the following characters if they are to represent themselves:

```
|  &  ;  <  >  (  )  $  `  \  "  '  <space>  <tab>  <newline>
```

and the following might need to be quoted under certain circumstances. That is, these characters are sometimes special depending on conditions described elsewhere in this volume of POSIX.1-2024:

```
*  ?  [  ]  ^  -  !  #  ~  =  %  {  ,  }
```

**Note:** A future version of this standard may extend the conditions under which these characters are special. Therefore applications should quote them whenever they are intended to represent themselves. This does not apply to `<hyphen-minus>` (`'-'`) since it is in the portable filename character set.

The various quoting mechanisms are the escape character, single-quotes, double-quotes, and dollar-single-quotes. The here-document represents another form of quoting; see [2.7.4 Here-Document](#274-here-document).

### Tests

#### Test: backslash-quoting preserves literal special characters

Backslash-quoting each of the mandatory-quote characters (`| & ; < > ( ) $ \` \ "`)
produces their literal values.

```
begin test "backslash-quoting preserves literal special characters"
  script
    echo \| \& \; \< \> \( \) \$ \` \\ \"
  expect
    stdout "\| & ; < > \( \) \$ ` \\ """
    stderr ""
    exit_code 0
end test "backslash-quoting preserves literal special characters"
```

#### Test: single-quoting preserves literal special characters

Single-quoting a string containing all mandatory-quote characters preserves each
one literally.

```
begin test "single-quoting preserves literal special characters"
  script
    echo '| & ; < > ( ) $ ` \ "'
  expect
    stdout "\| & ; < > \( \) \$ ` \\ """
    stderr ""
    exit_code 0
end test "single-quoting preserves literal special characters"
```

#### Test: double-quoting preserves literal pipe semicolon angle parens

Double-quoting preserves the literal value of `| & ; < > ( )`, which are not
special inside double-quotes.

```
begin test "double-quoting preserves literal pipe semicolon angle parens"
  script
    echo "| & ; < > ( )"
  expect
    stdout "\| & ; < > \( \)"
    stderr ""
    exit_code 0
end test "double-quoting preserves literal pipe semicolon angle parens"
```

#### Test: quoting preserves literal space and tab in single argument

Quoting spaces and tabs prevents field splitting, so `"hello world"` and `"a<TAB>b"`
each remain a single argument.

```
begin test "quoting preserves literal space and tab in single argument"
  script
    $SHELL -c 'for a in "$@"; do echo "[$a]"; done' sh "hello world" "a	b"
  expect
    stdout "\[hello world\]\n\[a	b\]"
    stderr ""
    exit_code 0
end test "quoting preserves literal space and tab in single argument"
```

#### Test: backslash-newline is line continuation not literal newline

A backslash immediately before a newline acts as line continuation, not as a
literal newline character.

```
begin test "backslash-newline is line continuation not literal newline"
  script
    echo hello\
    world
  expect
    stdout "helloworld"
    stderr ""
    exit_code 0
end test "backslash-newline is line continuation not literal newline"
```

#### Test: quoting prevents glob expansion of * ? [ ]

Quoting the conditionally-special glob characters `*`, `?`, `[` prevents
pathname expansion.

```
begin test "quoting prevents glob expansion of * ? [ ]"
  script
    echo '*' '?' '[abc]'
  expect
    stdout "\* \? \[abc\]"
    stderr ""
    exit_code 0
end test "quoting prevents glob expansion of * ? [ ]"
```

#### Test: quoting preserves literal ~ = % { } characters

The conditionally-special characters `~ = % { } , ^ -` are preserved literally
when quoted.

```
begin test "quoting preserves literal ~ = % { } characters"
  script
    echo '~' '=' '%' '{' '}' ',' '^' '-'
  expect
    stdout "~ = % { } , ^ -"
    stderr ""
    exit_code 0
end test "quoting preserves literal ~ = % { } characters"
```

#### Test: dollar-single-quote newline escape

The `$'...'` quoting mechanism processes backslash escapes; `$'a\nb'` produces a
literal newline between `a` and `b`.

```
begin test "dollar-single-quote newline escape"
  script
    echo $'a\nb'
  expect
    stdout "a\nb"
    stderr ""
    exit_code 0
end test "dollar-single-quote newline escape"
```

#### Test: quoted # is not a comment

The `#` character is conditionally special (it starts comments). When quoted
inside double-quotes, it is preserved literally and does not introduce a comment.

```
begin test "quoted # is not a comment"
  script
    echo "a # not a comment"
  expect
    stdout "a # not a comment"
    stderr ""
    exit_code 0
end test "quoted # is not a comment"
```

## 2.2.1 Escape Character (Backslash)

A `<backslash>` that is not quoted shall preserve the literal value of the following character, with the exception of a `<newline>`. If a `<newline>` immediately follows the `<backslash>`, the shell shall interpret this as line continuation. The `<backslash>` and `<newline>` shall be removed before splitting the input into tokens. Since the escaped `<newline>` is removed entirely from the input and is not replaced by any white space, it cannot serve as a token separator.

### Tests

#### Test: backslash preserves literal value of following character

A backslash before `*` preserves it literally inside a word, so `a\*b` produces
`a*b` without pathname expansion.

```
begin test "backslash preserves literal value of following character"
  script
    echo a\\*b
  expect
    stdout "a\\\*b"
    stderr ""
    exit_code 0
end test "backslash preserves literal value of following character"
```

#### Test: backslash escapes semicolon so it is literal

A backslash before `;` prevents the shell from treating it as a command
separator. The output is the literal string `foo;bar`.

```
begin test "backslash escapes semicolon so it is literal"
  script
    echo foo\;bar
  expect
    stdout "foo;bar"
    stderr ""
    exit_code 0
end test "backslash escapes semicolon so it is literal"
```

#### Test: backslash escapes space preventing field split

A backslash before a space prevents field splitting, so `foo\ bar` is a single
argument rather than two.

```
begin test "backslash escapes space preventing field split"
  script
    set -- foo\ bar
    printf "%s:%s\n" "$#" "$1"
  expect
    stdout "1:foo bar"
    stderr ""
    exit_code 0
end test "backslash escapes space preventing field split"
```

#### Test: backslash-newline is line continuation

When a newline immediately follows a backslash, the shell treats the pair as line
continuation. The token `ec` + `ho` is joined into the command `echo`.

```
begin test "backslash-newline is line continuation"
  script
    ec\
    ho line continuation
  expect
    stdout "line continuation"
    stderr ""
    exit_code 0
end test "backslash-newline is line continuation"
```

#### Test: backslash-newline line continuation between tokens

Line continuation between separate tokens: the backslash-newline after `echo`
joins it with `hello` on the next line.

```
begin test "backslash-newline line continuation between tokens"
  script
    echo \
    hello
  expect
    stdout "hello"
    stderr ""
    exit_code 0
end test "backslash-newline line continuation between tokens"
```

#### Test: multiple consecutive backslash-newline continuations

Multiple consecutive backslash-newline pairs are all removed, joining `ec`, an
empty continuation, and `ho multi` into `echo multi`.

```
begin test "multiple consecutive backslash-newline continuations"
  script
    ec\
    \
    ho multi
  expect
    stdout "multi"
    stderr ""
    exit_code 0
end test "multiple consecutive backslash-newline continuations"
```

#### Test: line continuation: backslash-newline removed before tokenizing

The backslash-newline pair is removed before token splitting, so `a\<newline>b`
becomes the single token `ab`.

```
begin test "line continuation: backslash-newline removed before tokenizing"
  script
    echo a\
    b
  expect
    stdout "ab"
    stderr ""
    exit_code 0
end test "line continuation: backslash-newline removed before tokenizing"
```

## 2.2.2 Single-Quotes

Enclosing characters in single-quotes (`''`) shall preserve the literal value of each character within the single-quotes. A single-quote cannot occur within single-quotes.

### Tests

#### Test: single quotes preserve all characters literally

Characters `$` and `*` inside single-quotes are not expanded; they appear in the
output as literal `$foo *`.

```
begin test "single quotes preserve all characters literally"
  script
    echo '$foo *'
  expect
    stdout "\$foo \*"
    stderr ""
    exit_code 0
end test "single quotes preserve all characters literally"
```

## 2.2.3 Double-Quotes

Enclosing characters in double-quotes (`""`) shall preserve the literal value of all characters within the double-quotes, with the exception of the characters backquote, `<dollar-sign>`, and `<backslash>`, as follows:

- `$`: The `<dollar-sign>` shall retain its special meaning introducing parameter expansion (see [2.6.2 Parameter Expansion](#262-parameter-expansion)), a form of command substitution (see [2.6.3 Command Substitution](#263-command-substitution)), and arithmetic expansion (see [2.6.4 Arithmetic Expansion](#264-arithmetic-expansion)), but shall not retain its special meaning introducing the dollar-single-quotes form of quoting (see [2.2.4 Dollar-Single-Quotes](#224-dollar-single-quotes)). The input characters within the quoted string that are also enclosed between `"$("` and the matching `')'` shall not be affected by the double-quotes, but rather shall define the command(s) whose output replaces the `"$(...)"` when the word is expanded. The tokenizing rules in [2.3 Token Recognition](#23-token-recognition) shall be applied recursively to find the matching `')'`. For the four varieties of parameter expansion that provide for substring processing (see [2.6.2 Parameter Expansion](#262-parameter-expansion)), within the string of characters from an enclosed `"${"` to the matching `'}'`, the double-quotes within which the expansion occurs shall have no effect on the handling of any special characters. For parameter expansions other than the four varieties that provide for substring processing, within the string of characters from an enclosed `"${"` to the matching `'}'`, the double-quotes within which the expansion occurs shall preserve the literal value of all characters, with the exception of the characters double-quote, backquote, `<dollar-sign>`, and `<backslash>`. If any unescaped double-quote characters occur within the string, other than in embedded command substitutions, the behavior is unspecified. The backquote and `<dollar-sign>` characters shall follow the same rules as for characters in double-quotes described in this section. The `<backslash>` character shall follow the same rules as for characters in double-quotes described in this section except that it shall additionally retain its special meaning as an escape character when followed by `'}'` and this shall prevent the escaped `'}'` from being considered when determining the matching `'}'` (using the rule in [2.6.2 Parameter Expansion](#262-parameter-expansion)).
- `` ` ``: The backquote shall retain its special meaning introducing the other form of command substitution (see [2.6.3 Command Substitution](#263-command-substitution)). The portion of the quoted string from the initial backquote and the characters up to the next backquote that is not preceded by a `<backslash>`, having escape characters removed, defines that command whose output replaces ``"`...`"`` when the word is expanded. Either of the following cases produces undefined results:

    - A quoted (single-quoted, double-quoted, or dollar-single-quoted) string that begins, but does not end, within the ``"`...`"`` sequence
    - A ``"`...`"`` sequence that begins, but does not end, within the same double-quoted string
- `\`: Outside of `"$(...)"` and `"${...}"` the `<backslash>` shall retain its special meaning as an escape character (see [2.2.1 Escape Character (Backslash)](#221-escape-character-backslash)) only when immediately followed by one of the following characters:

  ```
  $   `   \   <newline>
  ```

  or by a double-quote character that would otherwise be considered special (see [2.6.4 Arithmetic Expansion](#264-arithmetic-expansion) and [2.7.4 Here-Document](#274-here-document)).

When double-quotes are used to quote a parameter expansion, command substitution, or arithmetic expansion, the literal value of all characters within the result of the expansion shall be preserved.

The application shall ensure that a double-quote that is not within `"$(...)"` nor within `"${...}"` is immediately preceded by a `<backslash>` in order to be included within double-quotes. The parameter `'@'` has special meaning inside double-quotes and is described in [2.5.2 Special Parameters](#252-special-parameters).

### Tests

#### Test: double quotes allow parameter and command and arithmetic expansion

Inside double-quotes, `$foo`, `$(echo sub)`, and `$((2+2))` are expanded, but
`$'literal'` (dollar-single-quote) is not special.

```
begin test "double quotes allow parameter and command and arithmetic expansion"
  script
    foo=bar
    echo "$foo $(echo sub) $((2+2)) $'literal'"
  expect
    stdout "bar sub 4 \$'literal'"
    stderr ""
    exit_code 0
end test "double quotes allow parameter and command and arithmetic expansion"
```

#### Test: inner double quotes inside command substitution

Double-quotes inside `$(...)` are independent from the outer double-quotes; the
inner `"inner quotes"` is processed as a separate quoting context.

```
begin test "inner double quotes inside command substitution"
  script
    echo "$(echo "inner quotes")"
  expect
    stdout "inner quotes"
    stderr ""
    exit_code 0
end test "inner double quotes inside command substitution"
```

#### Test: recursive tokenizing finds matching paren

The shell applies tokenizing rules recursively to find the matching `)` for
`$(...)`, even when the inner command contains parentheses.

```
begin test "recursive tokenizing finds matching paren"
  script
    echo "$(echo "(recursive)")"
  expect
    stdout "\(recursive\)"
    stderr ""
    exit_code 0
end test "recursive tokenizing finds matching paren"
```

#### Test: backquote inside double quotes executes

A backquote inside double-quotes retains its special meaning and introduces
command substitution.

```
begin test "backquote inside double quotes executes"
  script
    echo "`echo sub`"
  expect
    stdout "sub"
    stderr ""
    exit_code 0
end test "backquote inside double quotes executes"
```

#### Test: backslash in double quotes special only before certain chars

Inside double-quotes (outside `$(...)` and `${...}`), backslash is only special
before `$`, `` ` ``, `"`, `\`, and newline. Before other characters like `n`, the
backslash is preserved literally.

```
begin test "backslash in double quotes special only before certain chars"
  script
    printf "%s\n" "\n \$ \` \\"
  expect
    stdout "\\n \$ ` \\"
    stderr ""
    exit_code 0
end test "backslash in double quotes special only before certain chars"
```

#### Test: double quotes preserve expansion result literally

When a parameter expansion occurs inside double-quotes, the result is preserved
literally without further expansion — glob characters in the value are not
expanded.

```
begin test "double quotes preserve expansion result literally"
  script
    foo='* * *'
    echo "$foo"
  expect
    stdout "\* \* \*"
    stderr ""
    exit_code 0
end test "double quotes preserve expansion result literally"
```

#### Test: substring processing not affected by outer double quotes

For substring-processing parameter expansions (`${var#pattern}`, etc.), the
double-quotes have no effect on pattern matching inside the braces.

```
begin test "substring processing not affected by outer double quotes"
  script
    foo="a*b"
    unset unset_var
    echo "${foo#a*}" "${unset_var:-*}"
  expect
    stdout ".*\*b \*.*"
    stderr ""
    exit_code 0
end test "substring processing not affected by outer double quotes"
```

#### Test: backslash dollar and backquote inside braces

Inside `${...}`, backquote and `$` retain their special meanings (command
substitution and expansion), while `\` escapes `$` and follows double-quote
rules.

```
begin test "backslash dollar and backquote inside braces"
  script
    unset foo
    printf "%s\n" "${foo:-`echo default` \$ \n \\ }"
  expect
    stdout "default \$ \\n \\.*"
    stderr ""
    exit_code 0
end test "backslash dollar and backquote inside braces"
```

#### Test: double quotes prevent wildcard expansion

Glob characters inside double-quotes are not expanded; `"a*b"` outputs the
literal string `a*b`.

```
begin test "double quotes prevent wildcard expansion"
  script
    echo "a*b"
  expect
    stdout "a\*b"
    stderr ""
    exit_code 0
end test "double quotes prevent wildcard expansion"
```

#### Test: double quotes backslash produces single backslash

Inside double-quotes, `\\` (backslash before backslash) produces a single
literal backslash.

```
begin test "double quotes backslash produces single backslash"
  script
    echo "\\"
  expect
    stdout "\\"
    stderr ""
    exit_code 0
end test "double quotes backslash produces single backslash"
```

#### Test: escaped double quote inside double quotes

A backslash before `"` inside double-quotes produces a literal double-quote
character, as required for including `"` within double-quoted strings.

```
begin test "escaped double quote inside double quotes"
  script
    echo "\""
  expect
    stdout """"
    stderr ""
    exit_code 0
end test "escaped double quote inside double quotes"
```

#### Test: dollar-paren command substitution

The `$(...)` command substitution form works inside double-quotes, replacing the
construct with the command's standard output.

```
begin test "dollar-paren command substitution"
  script
    echo $(echo hello)
  expect
    stdout "hello"
    stderr ""
    exit_code 0
end test "dollar-paren command substitution"
```

## 2.2.4 Dollar-Single-Quotes

A sequence of characters starting with a `<dollar-sign>` immediately followed by a single-quote (`$'`) shall preserve the literal value of all characters up to an unescaped terminating single-quote (`'`), with the exception of certain `<backslash>`-escape sequences, as follows:

- `\"` yields a `<quotation-mark>` (double-quote) character, but note that `<quotation-mark>` can be included unescaped.
- `\'` yields an `<apostrophe>` (single-quote) character.
- `\\` yields a `<backslash>` character.
- `\a` yields an `<alert>` character.
- `\b` yields a `<backspace>` character.
- `\e` yields an `<ESC>` character.
- `\f` yields a `<form-feed>` character.
- `\n` yields a `<newline>` character.
- `\r` yields a `<carriage-return>` character.
- `\t` yields a `<tab>` character.
- `\v` yields a `<vertical-tab>` character.
- `\c`*X* yields the control character listed in the **Value** column of [*Values for cpio c_mode Field*](../utilities/stty.md#tagtcjh_23) in the OPERANDS section of the [*stty*](../utilities/stty.md) utility when *X* is one of the characters listed in the **^c** column of the same table, except that `\c\\` yields the `<FS>` control character since the `<backslash>` character has to be escaped.
- `\x`*XX* yields the byte whose value is the hexadecimal value *XX* (one or more hexadecimal digits). If more than two hexadecimal digits follow `\x`, the results are unspecified.
- `\`*ddd* yields the byte whose value is the octal value *ddd* (one to three octal digits).
- The behavior of an unescaped `<backslash>` immediately followed by any other character, including `<newline>`, is unspecified.

In cases where a variable number of characters can be used to specify an escape sequence (`\x`*XX* and `\`*ddd*), the escape sequence shall be terminated by the first character that is not of the expected type or, for `\`*ddd* sequences, when the maximum number of characters specified has been found, whichever occurs first.

These `<backslash>`-escape sequences shall be processed (replaced with the bytes or characters they yield) immediately prior to word expansion (see [2.6 Word Expansions](#26-word-expansions)) of the word in which the dollar-single-quotes sequence occurs.

If a `\x`*XX* or `\`*ddd* escape sequence yields a byte whose value is 0, it is unspecified whether that null byte is included in the result or if that byte and any following regular characters and escape sequences up to the terminating unescaped single-quote are evaluated and discarded.

If the octal value specified by `\`*ddd* will not fit in a byte, the results are unspecified.

If a `\e` or `\c`*X* escape sequence specifies a character that does not have an encoding in the locale in effect when these `<backslash>`-escape sequences are processed, the result is implementation-defined. However, implementations shall not replace an unsupported character with bytes that do not form valid characters in that locale's character set.

If a `<backslash>`-escape sequence represents a single-quote character (for example `\'`), that sequence shall not terminate the dollar-single-quote sequence.

### Tests

#### Test: dollar-single-quote basic support

The `$'...'` syntax processes `\n` as a newline character, producing two lines
of output.

```
begin test "dollar-single-quote basic support"
  script
    printf '%s\n' $'hello\nworld'
  expect
    stdout "hello\nworld"
    stderr ""
    exit_code 0
end test "dollar-single-quote basic support"
```

#### Test: dollar-single-quote hex escape

The `\xHH` escape in `$'...'` produces the byte with the given hexadecimal value;
`\x41\x42` yields `AB`.

```
begin test "dollar-single-quote hex escape"
  script
    echo $'\x41\x42'
  expect
    stdout "AB"
    stderr ""
    exit_code 0
end test "dollar-single-quote hex escape"
```

#### Test: dollar-single-quote escaped single quote

The `\'` escape inside `$'...'` produces a literal single-quote without
terminating the dollar-single-quote sequence.

```
begin test "dollar-single-quote escaped single quote"
  script
    echo $'can\'t'
  expect
    stdout "can't"
    stderr ""
    exit_code 0
end test "dollar-single-quote escaped single quote"
```

#### Test: dollar-single-quote variable-length escapes terminate correctly

Variable-length escape sequences (`\xHH` and `\ddd`) terminate at the first
character that is not of the expected type, or when the maximum number of digits
has been consumed.

```
begin test "dollar-single-quote variable-length escapes terminate correctly"
  script
    printf "%s|%s|%s|%s\n" $'\x41' $'\x41Z' $'\101' $'\1012'
  expect
    stdout "A\|AZ\|A\|A2"
    stderr ""
    exit_code 0
end test "dollar-single-quote variable-length escapes terminate correctly"
```
