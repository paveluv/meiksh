# 2.2 Quoting

This test suite covers **Section 2.2 Quoting** of the POSIX Shell Command Language
(POSIX.1-2024), which defines how the shell interprets special characters and the
four quoting mechanisms: the escape character (backslash), single-quotes,
double-quotes, and dollar-single-quotes.

## Standard Text

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

## Tests

### 2.2 Quoting (General)

The preamble lists characters that must be quoted to represent themselves. These
tests verify that all three quoting mechanisms (backslash, single-quotes,
double-quotes) correctly preserve these characters as literals.

##### Test: backslash-quoting preserves literal special characters

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

##### Test: single-quoting preserves literal special characters

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

##### Test: double-quoting preserves literal pipe semicolon angle parens

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

##### Test: quoting preserves literal space and tab in single argument

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

##### Test: backslash-newline is line continuation not literal newline

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

##### Test: quoting prevents glob expansion of * ? [ ]

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

##### Test: quoting preserves literal ~ = % { } characters

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

##### Test: dollar-single-quote newline escape

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

##### Test: quoted # is not a comment

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

### 2.2.1 Escape Character (Backslash)

An unquoted backslash preserves the literal value of the next character, except
that backslash-newline is treated as line continuation (the pair is removed before
tokenizing).

##### Test: backslash preserves literal value of following character

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

##### Test: backslash escapes semicolon so it is literal

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

##### Test: backslash escapes space preventing field split

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

##### Test: backslash-newline is line continuation

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

##### Test: backslash-newline line continuation between tokens

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

##### Test: multiple consecutive backslash-newline continuations

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

##### Test: line continuation: backslash-newline removed before tokenizing

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

### 2.2.2 Single-Quotes

Single-quotes preserve the literal value of every character within them. A
single-quote cannot occur within single-quotes.

##### Test: single quotes preserve all characters literally

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

### 2.2.3 Double-Quotes

Double-quotes preserve the literal value of all characters except backquote, `$`,
and `\`, which retain their special meanings for expansions and escaping.

##### Test: double quotes allow parameter and command and arithmetic expansion

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

##### Test: inner double quotes inside command substitution

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

##### Test: recursive tokenizing finds matching paren

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

##### Test: backquote inside double quotes executes

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

##### Test: backslash in double quotes special only before certain chars

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

##### Test: double quotes preserve expansion result literally

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

##### Test: substring processing not affected by outer double quotes

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

##### Test: backslash dollar and backquote inside braces

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

##### Test: double quotes prevent wildcard expansion

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

##### Test: double quotes backslash produces single backslash

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

##### Test: escaped double quote inside double quotes

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

##### Test: dollar-paren command substitution

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

### 2.2.4 Dollar-Single-Quotes

The `$'...'` quoting mechanism preserves all characters literally except for
certain backslash-escape sequences (`\n`, `\t`, `\xHH`, `\ddd`, `\'`, etc.).

##### Test: dollar-single-quote basic support

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

##### Test: dollar-single-quote hex escape

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

##### Test: dollar-single-quote escaped single quote

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

##### Test: dollar-single-quote variable-length escapes terminate correctly

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
