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

Backslash-quoting preserves the literal value of each following character. This
test covers representative shell metacharacters and quote characters using
`printf` so the result is not affected by `echo`'s implementation-defined
backslash handling.

```
begin test "backslash-quoting preserves literal special characters"
  script
    printf '%s\n' \| \& \; \< \> \( \) \$ \` \\ \" \'
  expect
    stdout "\|\n&\n;\n<\n>\n\(\n\)\n\$\n`\n\\\n""\n'"
    stderr ""
    exit_code 0
end test "backslash-quoting preserves literal special characters"
```

#### Test: single-quoting preserves literal shell metacharacters

Single-quotes preserve the literal value of every character inside them. This
test covers representative metacharacters, parameter-expansion syntax,
command-substitution syntax, pathname-expansion syntax, a comment introducer,
and a backslash.

```
begin test "single-quoting preserves literal shell metacharacters"
  script
    printf '%s\n' '| & ; < > ( ) $ ` \ " * ? [abc] #'
  expect
    stdout "\| & ; < > \( \) \$ ` \\ "" \* \? \[abc\] #"
    stderr ""
    exit_code 0
end test "single-quoting preserves literal shell metacharacters"
```

#### Test: double-quoting preserves literal pipe semicolon angle parens

Double-quoting preserves the literal value of `| & ; < > ( )`, which are not
special inside double-quotes.

```
begin test "double-quoting preserves literal pipe semicolon angle parens"
  script
    printf '%s\n' "| & ; < > ( )"
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
    printf '%s\n' '*' '?' '[abc]'
  expect
    stdout "\*\n\?\n\[abc\]"
    stderr ""
    exit_code 0
end test "quoting prevents glob expansion of * ? [ ]"
```

#### Test: quoting preserves literal ! and ] characters

The conditionally-special characters `!` and `]` are preserved literally when
quoted.

```
begin test "quoting preserves literal ! and ] characters"
  script
    printf '%s\n' '!' ']'
  expect
    stdout "!\n\]"
    stderr ""
    exit_code 0
end test "quoting preserves literal ! and ] characters"
```

#### Test: quoting preserves literal ~ = % { } , ^ - characters

The conditionally-special characters `~ = % { } , ^ -` are preserved literally
when quoted.

```
begin test "quoting preserves literal ~ = % { } , ^ - characters"
  script
    printf '%s\n' '~' '=' '%' '{' '}' ',' '^' '-'
  expect
    stdout "~\n=\n%\n\{\n\}\n,\n\^\n-"
    stderr ""
    exit_code 0
end test "quoting preserves literal ~ = % { } , ^ - characters"
```

#### Test: dollar-single-quote newline escape

The `$'...'` quoting mechanism processes backslash escapes; `$'a\nb'` produces a
literal newline between `a` and `b`.

```
begin test "dollar-single-quote newline escape"
  script
    printf '%s\n' $'a\nb'
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
    printf '%s\n' "a # not a comment"
  expect
    stdout "a # not a comment"
    stderr ""
    exit_code 0
end test "quoted # is not a comment"
```

#### Test: adjacent quoting forms concatenate into single word

Different quoting mechanisms placed next to each other without intervening
whitespace are joined into a single token. This is essential for embedding
characters that cannot appear inside a particular quoting form (for example, a
single-quote inside a single-quoted region).

```
begin test "adjacent quoting forms concatenate into single word"
  script
    set -- 'foo'"bar" "hello "world 'a'\''b'
    printf '%s\n%s\n%s\n%s\n' "$#" "$1" "$2" "$3"
  expect
    stdout "3\nfoobar\nhello world\na'b"
    stderr ""
    exit_code 0
end test "adjacent quoting forms concatenate into single word"
```

#### Test: adjacent dollar-single-quote concatenates with other forms

The dollar-single-quote form is just another quoting mechanism and participates
in the same concatenation rule. Placing `$'...'` next to a single-quoted and a
bare word without whitespace yields a single token.

```
begin test "adjacent dollar-single-quote concatenates with other forms"
  script
    set -- 'foo'$'\x2d'bar
    printf '%s\n%s\n' "$#" "$1"
  expect
    stdout "1\nfoo-bar"
    stderr ""
    exit_code 0
end test "adjacent dollar-single-quote concatenates with other forms"
```

#### Test: quoting prevents reserved word recognition

Quoting can prevent a reserved word from being recognized as such. Here the
quoted word `'if'` is treated as a command name, allowing an executable named
`if` to run.

```
begin test "quoting prevents reserved word recognition"
  script
    cat > if <<'EOF'
    #!/bin/sh
    printf '%s\n' reserved-word-suppressed
    EOF
    chmod +x if
    PATH=".:$PATH" 'if'
  expect
    stdout "reserved-word-suppressed"
    stderr ""
    exit_code 0
end test "quoting prevents reserved word recognition"
```

#### Test: quoted here-doc delimiter suppresses expansion

Quoting a here-document delimiter prevents parameter expansion and command
substitution in the here-document body.

```
begin test "quoted here-doc delimiter suppresses expansion"
  script
    value=world
    cat <<'EOF'
    hello $value
    $(printf '%s' cmd)
    EOF
  expect
    stdout "hello \$value\n\$\(printf '%s' cmd\)"
    stderr ""
    exit_code 0
end test "quoted here-doc delimiter suppresses expansion"
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
    printf '%s\n' a\*b
  expect
    stdout "a\*b"
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
    printf '%s\n' foo\;bar
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

#### Test: backslash before dollar prevents parameter expansion

A backslash before `$` preserves the dollar sign literally, preventing it from
introducing parameter expansion.

```
begin test "backslash before dollar prevents parameter expansion"
  script
    HOME=/should/not/appear
    printf '%s\n' \$HOME
  expect
    stdout "\$HOME"
    stderr ""
    exit_code 0
end test "backslash before dollar prevents parameter expansion"
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

When a backslash-newline appears between tokens, both characters are removed.
The next word remains separated by the remaining space, so this command still
has `echo` and `hello` as separate tokens.

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

#### Test: backslash before ordinary character yields character literally

An unquoted backslash preserves the literal value of the following character.
When the following character is not otherwise special, the backslash is simply
removed and the character is retained. Here `\a\b\c` becomes `abc`.

```
begin test "backslash before ordinary character yields character literally"
  script
    printf '%s\n' \a\b\c
  expect
    stdout "abc"
    stderr ""
    exit_code 0
end test "backslash before ordinary character yields character literally"
```

## 2.2.2 Single-Quotes

Enclosing characters in single-quotes (`''`) shall preserve the literal value of each character within the single-quotes. A single-quote cannot occur within single-quotes.

### Tests

#### Test: single quotes suppress parameter command and pathname expansion

A single-quoted word suppresses parameter expansion, command substitution, and
pathname expansion. The characters `$foo`, `$(printf cmd)`, and `*` therefore
remain literal.

```
begin test "single quotes suppress parameter command and pathname expansion"
  script
    printf '%s\n' '$foo' '$(printf cmd)' '*'
  expect
    stdout "\$foo\n\$\(printf cmd\)\n\*"
    stderr ""
    exit_code 0
end test "single quotes suppress parameter command and pathname expansion"
```

#### Test: single quotes preserve spaces tabs and semicolons literally

Single-quotes preserve embedded spaces, tabs, and semicolons as ordinary
characters, so each quoted word remains one shell argument.

```
begin test "single quotes preserve spaces tabs and semicolons literally"
  script
    set -- 'a b' 'c	d' 'x;y'
    printf '%s\n%s\n%s\n%s\n' "$#" "$1" "$2" "$3"
  expect
    stdout "3\na b\nc	d\nx;y"
    stderr ""
    exit_code 0
end test "single quotes preserve spaces tabs and semicolons literally"
```

#### Test: single quotes preserve literal newline

Single-quotes preserve the literal value of every character inside them,
including a newline.

```
begin test "single quotes preserve literal newline"
  script
    printf '%s\n' 'hello
    world'
  expect
    stdout "hello\nworld"
    stderr ""
    exit_code 0
end test "single quotes preserve literal newline"
```

#### Test: single quotes preserve literal backslash

Inside single-quotes, backslash has no special meaning and is preserved
literally along with any characters that would otherwise be an escape outside
quotes. The string `'a\nb'` therefore has four characters: `a`, `\`, `n`, `b`.

```
begin test "single quotes preserve literal backslash"
  script
    printf '%s\n' 'a\nb'
  expect
    stdout "a\\nb"
    stderr ""
    exit_code 0
end test "single quotes preserve literal backslash"
```

#### Test: single-quote cannot occur within single-quotes

A single-quote cannot appear inside single-quotes because there is no escape
mechanism. To include a literal single-quote, end the single-quoted string,
insert an escaped or differently-quoted single-quote, and resume.

```
begin test "single-quote cannot occur within single-quotes"
  script
    printf '%s\n' 'don'\''t stop'
  expect
    stdout "don't stop"
    stderr ""
    exit_code 0
end test "single-quote cannot occur within single-quotes"
```

#### Test: unterminated single quote causes shell syntax error

An unmatched single-quote is not a valid shell token. The shell should reject
the script with a syntax error instead of executing it.

```
begin test "unterminated single quote causes shell syntax error"
  script
    printf '%s\n' 'unterminated
  expect
    stdout ""
    stderr "(.|\n)+"
    exit_code !=0
end test "unterminated single quote causes shell syntax error"
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
    printf '%s\n' "$foo $(echo sub) $((2+2)) $'literal'"
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
    printf '%s\n' "$(printf '%s\n' "inner quotes")"
  expect
    stdout "inner quotes"
    stderr ""
    exit_code 0
end test "inner double quotes inside command substitution"
```

#### Test: inner single quotes inside command substitution

The characters inside `"$(...)"` are not affected by the outer double-quotes.
A single-quoted string inside `$(...)` therefore begins its own quoting
context even though the `'` would be an ordinary character in a plain
double-quoted string.

```
begin test "inner single quotes inside command substitution"
  script
    printf '%s\n' "$(printf '%s' 'inner-sq')"
  expect
    stdout "inner-sq"
    stderr ""
    exit_code 0
end test "inner single quotes inside command substitution"
```

#### Test: recursive tokenizing finds matching paren

The shell applies tokenizing rules recursively to find the matching `)` for
`$(...)`, even when the inner command contains parentheses.

```
begin test "recursive tokenizing finds matching paren"
  script
    printf '%s\n' "$(printf '%s\n' "(recursive)")"
  expect
    stdout "\(recursive\)"
    stderr ""
    exit_code 0
end test "recursive tokenizing finds matching paren"
```

#### Test: nested subshell parens do not close command substitution

When a subshell `(...)` appears inside `$(...)`, the shell applies token
recognition recursively. The `)` that closes the subshell must not be mistaken
for the `)` that closes the command substitution.

```
begin test "nested subshell parens do not close command substitution"
  script
    printf '%s\n' "$(echo a; (echo b))"
  expect
    stdout "a\nb"
    stderr ""
    exit_code 0
end test "nested subshell parens do not close command substitution"
```

#### Test: backquote inside double quotes executes

A backquote inside double-quotes retains its special meaning and introduces
command substitution.

```
begin test "backquote inside double quotes executes"
  script
    printf '%s\n' "`printf '%s\n' sub`"
  expect
    stdout "sub"
    stderr ""
    exit_code 0
end test "backquote inside double quotes executes"
```

#### Test: backslash escapes removed inside double-quoted backquote

Inside double-quotes, the command string of a backquote command substitution is
formed by removing backslash escapes before passing the string to the subshell.
Here `\$` is passed as `$` and `\"` is passed as `"`.

```
begin test "backslash escapes removed inside double-quoted backquote"
  script
    foo=outer
    printf '%s\n' "`foo=inner; printf \"%s\" \"\$foo\"`"
  expect
    stdout "inner"
    stderr ""
    exit_code 0
end test "backslash escapes removed inside double-quoted backquote"
```

#### Test: backslash before non-special character stays literal in double quotes

Inside double-quotes, backslash is only special before `$`, `` ` ``, `"`, `\`,
and newline. Before other characters like `n`, `a`, and `*`, the backslash is
preserved literally.

```
begin test "backslash before non-special character stays literal in double quotes"
  script
    printf '%s\n' "\n" "\a" "\*"
  expect
    stdout "\\n\n\\a\n\\\*"
    stderr ""
    exit_code 0
end test "backslash before non-special character stays literal in double quotes"
```

#### Test: backslash escapes special characters in double quotes

Inside double-quotes, backslash escapes `$`, `` ` ``, `"`, and `\`, yielding the
literal characters without leaving the backslash in the result.

```
begin test "backslash escapes special characters in double quotes"
  script
    printf '%s\n' "\$" "\`" "\"" "\\"
  expect
    stdout "\$\n`\n""\n\\"
    stderr ""
    exit_code 0
end test "backslash escapes special characters in double quotes"
```

#### Test: backslash before dollar in double quotes prevents parameter expansion

Inside double-quotes, a backslash before `$` escapes the dollar sign so it does
not introduce parameter expansion. The variable `HOME` is set but `"\$HOME"`
still expands to the literal four-character string `$HOME`.

```
begin test "backslash before dollar in double quotes prevents parameter expansion"
  script
    HOME=/should/not/appear
    printf '%s\n' "\$HOME"
  expect
    stdout "\$HOME"
    stderr ""
    exit_code 0
end test "backslash before dollar in double quotes prevents parameter expansion"
```

#### Test: backslash-newline inside double quotes is continuation

Inside double-quotes, a backslash immediately followed by newline is still a
line continuation. Both characters are removed from the resulting word.

```
begin test "backslash-newline inside double quotes is continuation"
  script
    script=$'printf \'%s\\n\' "ab\\\ncd"\n'
    $SHELL -c "$script"
  expect
    stdout "abcd"
    stderr ""
    exit_code 0
end test "backslash-newline inside double quotes is continuation"
```

#### Test: double quotes preserve literal newline

Newline is not one of the characters exempted from literal preservation inside
double-quotes, so an embedded newline is preserved as part of the quoted field.

```
begin test "double quotes preserve literal newline"
  script
    printf '%s\n' "hello
    world"
  expect
    stdout "hello\nworld"
    stderr ""
    exit_code 0
end test "double quotes preserve literal newline"
```

#### Test: double quotes preserve parameter expansion as one field

When parameter expansion occurs inside double-quotes, the resulting text remains
a single field and pathname expansion is not applied to the expansion result.

```
begin test "double quotes preserve parameter expansion as one field"
  script
    foo='a b *'
    set -- "$foo"
    printf '%s\n%s\n' "$#" "$1"
  expect
    stdout "1\na b \*"
    stderr ""
    exit_code 0
end test "double quotes preserve parameter expansion as one field"
```

#### Test: double quotes preserve command substitution as one field

When command substitution occurs inside double-quotes, the resulting text remains
a single field and pathname expansion is not applied to the substitution result.

```
begin test "double quotes preserve command substitution as one field"
  script
    set -- "$(printf '%s' 'a b *')"
    printf '%s\n%s\n' "$#" "$1"
  expect
    stdout "1\na b \*"
    stderr ""
    exit_code 0
end test "double quotes preserve command substitution as one field"
```

#### Test: double quotes preserve arithmetic expansion as one field

When arithmetic expansion occurs inside double-quotes, the resulting text is
preserved as one field.

```
begin test "double quotes preserve arithmetic expansion as one field"
  script
    set -- "$((1 + 2))"
    printf '%s\n%s\n' "$#" "$1"
  expect
    stdout "1\n3"
    stderr ""
    exit_code 0
end test "double quotes preserve arithmetic expansion as one field"
```

#### Test: quoted $@ preserves positional parameter boundaries

Inside double-quotes, `"$@"` expands to separate fields preserving the original
boundaries of the positional parameters.

```
begin test "quoted $@ preserves positional parameter boundaries"
  script
    set -- 'a b' c
    for x in "$@"; do printf '[%s]\n' "$x"; done
  expect
    stdout "\[a b\]\n\[c\]"
    stderr ""
    exit_code 0
end test "quoted $@ preserves positional parameter boundaries"
```

#### Test: parameter expansion active inside substring pattern in double quotes

For the four substring-processing parameter expansion forms, the enclosing
double-quotes have no effect on the handling of special characters inside the
braces, so a `$`-introduced parameter expansion in the pattern word is still
performed. With `v='abc'` and `p='a'`, `"${v#$p}"` expands to `bc`.

```
begin test "parameter expansion active inside substring pattern in double quotes"
  script
    v='abc'
    p='a'
    printf '%s\n' "${v#$p}"
  expect
    stdout "bc"
    stderr ""
    exit_code 0
end test "parameter expansion active inside substring pattern in double quotes"
```

#### Test: double quotes preserve conditionally-special characters literally

Inside double-quotes, the conditionally-special characters `~ = % { } , ^ - !`
are all preserved literally. Glob/path characters are already covered by a
separate test; this covers the remaining set.

```
begin test "double quotes preserve conditionally-special characters literally"
  script
    printf '%s\n' "~" "=" "%" "{" "}" "," "^" "-" "!"
  expect
    stdout "~\n=\n%\n\{\n\}\n,\n\^\n-\n!"
    stderr ""
    exit_code 0
end test "double quotes preserve conditionally-special characters literally"
```

#### Test: all four substring processing forms active in double quotes

For the four substring-processing parameter expansion forms (`#`, `##`, `%`,
`%%`), the outer double-quotes do not disable pattern syntax inside the braces.
The `*` glob character in each pattern still matches as expected.

```
begin test "all four substring processing forms active in double quotes"
  script
    v='aXbXcXd'
    printf '%s\n' "${v#*X}" "${v##*X}" "${v%X*}" "${v%%X*}"
  expect
    stdout "bXcXd\nd\naXbXc\na"
    stderr ""
    exit_code 0
end test "all four substring processing forms active in double quotes"
```

#### Test: default word remains literal inside quoted ${...:-word}

For parameter expansions other than the substring-processing forms, the outer
double-quotes preserve the literal value of ordinary characters in the default
word. The `*` here remains literal.

```
begin test "default word remains literal inside quoted ${...:-word}"
  script
    unset foo
    printf '%s\n' "${foo:-*}"
  expect
    stdout "\*"
    stderr ""
    exit_code 0
end test "default word remains literal inside quoted ${...:-word}"
```

#### Test: command substitution remains active inside quoted ${...:-word}

Inside `${...}` within double-quotes, `$(` still introduces command
substitution for non-substring expansions.

```
begin test "command substitution remains active inside quoted ${...:-word}"
  script
    unset foo
    printf '%s\n' "${foo:-$(printf '%s\n' default)}"
  expect
    stdout "default"
    stderr ""
    exit_code 0
end test "command substitution remains active inside quoted ${...:-word}"
```

#### Test: backquote substitution active inside quoted ${...:-word}

The standard says the backquote and dollar-sign follow the same rules inside a
non-substring `${...}` inside double-quotes as they would anywhere else inside
double-quotes. A backquote command substitution in the default word therefore
executes and its output replaces the backquoted region.

```
begin test "backquote substitution active inside quoted ${...:-word}"
  script
    unset foo
    printf '%s\n' "${foo:-`printf '%s' BT`}"
  expect
    stdout "BT"
    stderr ""
    exit_code 0
end test "backquote substitution active inside quoted ${...:-word}"
```

#### Test: escaped right brace does not terminate quoted ${...}

Inside `${...}` within double-quotes, a backslash before `}` prevents that
character from being treated as the closing brace for the expansion.

```
begin test "escaped right brace does not terminate quoted ${...}"
  script
    unset foo
    printf '%s\n' "${foo:-\}x}"
  expect
    stdout "\}x"
    stderr ""
    exit_code 0
end test "escaped right brace does not terminate quoted ${...}"
```

#### Test: double quotes prevent wildcard expansion

Glob characters inside double-quotes are not expanded; `"a*b"` outputs the
literal string `a*b`.

```
begin test "double quotes prevent wildcard expansion"
  script
    printf '%s\n' "a*b"
  expect
    stdout "a\*b"
    stderr ""
    exit_code 0
end test "double quotes prevent wildcard expansion"
```

#### Test: escaped double quote inside double quotes

A backslash before `"` inside double-quotes produces a literal double-quote
character, as required for including `"` within double-quoted strings.

```
begin test "escaped double quote inside double quotes"
  script
    printf '%s\n' "\""
  expect
    stdout """"
    stderr ""
    exit_code 0
end test "escaped double quote inside double quotes"
```

#### Test: unterminated double quote causes shell syntax error

An unmatched double-quote is not valid shell syntax. The shell should reject the
script with a syntax error.

```
begin test "unterminated double quote causes shell syntax error"
  script
    printf '%s\n' "unterminated
  expect
    stdout ""
    stderr "(.|\n)+"
    exit_code !=0
end test "unterminated double quote causes shell syntax error"
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
- `\c`*X* yields the control character listed in the **Value** column of [*Values for cpio c_mode Field*](docs/posix/md/utilities/stty.md#tagtcjh_23) in the OPERANDS section of the [*stty*](docs/posix/md/utilities/stty.md) utility when *X* is one of the characters listed in the **^c** column of the same table, except that `\c\\` yields the `<FS>` control character since the `<backslash>` character has to be escaped.
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
`\x41\x42` yields `AB` and `\xA` yields the newline byte `0x0a`.

```
begin test "dollar-single-quote hex escape"
  script
    printf '%s' $'\x41\x42\xA' | od -An -tx1
  expect
    stdout " *41 +42 +0a"
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
    printf '%s\n' $'can\'t'
  expect
    stdout "can't"
    stderr ""
    exit_code 0
end test "dollar-single-quote escaped single quote"
```

#### Test: dollar-single-quote double-quote escape

The `\"` escape yields a literal double-quote character inside `$'...'`.

```
begin test "dollar-single-quote double-quote escape"
  script
    printf '%s\n' $'a\"b'
  expect
    stdout "a""b"
    stderr ""
    exit_code 0
end test "dollar-single-quote double-quote escape"
```

#### Test: dollar-single-quote allows unescaped double quote

A double-quote can also appear unescaped inside `$'...'`; it does not terminate
the dollar-single-quoted sequence.

```
begin test "dollar-single-quote allows unescaped double quote"
  script
    printf '%s\n' $'a"b'
  expect
    stdout "a""b"
    stderr ""
    exit_code 0
end test "dollar-single-quote allows unescaped double quote"
```

#### Test: dollar-single-quote backslash escape

The `\\` escape yields a literal backslash character inside `$'...'`.

```
begin test "dollar-single-quote backslash escape"
  script
    printf '%s\n' $'a\\b'
  expect
    stdout "a\\b"
    stderr ""
    exit_code 0
end test "dollar-single-quote backslash escape"
```

#### Test: dollar-single-quote octal escape

The `\ddd` form accepts one to three octal digits and yields the corresponding
byte value. `\101\102` therefore produces `AB`.

```
begin test "dollar-single-quote octal escape"
  script
    printf '%s\n' $'\101\102'
  expect
    stdout "AB"
    stderr ""
    exit_code 0
end test "dollar-single-quote octal escape"
```

#### Test: dollar-single-quote control escapes produce expected bytes

The named control escapes `\a`, `\b`, `\e`, `\f`, `\r`, `\t`, and `\v` yield
their specified byte values.

```
begin test "dollar-single-quote control escapes produce expected bytes"
  script
    printf '%s' $'\a\b\e\f\r\t\v' | od -An -tx1
  expect
    stdout " *07 +08 +1b +0c +0d +09 +0b"
    stderr ""
    exit_code 0
end test "dollar-single-quote control escapes produce expected bytes"
```

#### Test: dollar-single-quote c-control escape

The `\cX` form yields the corresponding control character. For `X=A`, the byte
value is `0x01`.

```
begin test "dollar-single-quote c-control escape"
  script
    printf '%s' $'\cA' | od -An -tx1
  expect
    stdout " *01"
    stderr ""
    exit_code 0
end test "dollar-single-quote c-control escape"
```

#### Test: dollar-single-quote c-backslash yields fs

The special case `\c\\` yields the file-separator control character (`0x1c`).

```
begin test "dollar-single-quote c-backslash yields fs"
  script
    printf '%s' $'\c\\' | od -An -tx1
  expect
    stdout " *1c"
    stderr ""
    exit_code 0
end test "dollar-single-quote c-backslash yields fs"
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

#### Test: dollar-single-quote escapes are processed before word expansion

Backslash escapes inside `$'...'` are processed immediately before word
expansion, but the resulting characters remain literal as part of the quoted
word. An escaped dollar sign therefore does not start a new parameter
expansion.

```
begin test "dollar-single-quote escapes are processed before word expansion"
  script
    foo=BAR
    printf '%s\n' $'\x24'foo
  expect
    stdout "\$foo"
    stderr ""
    exit_code 0
end test "dollar-single-quote escapes are processed before word expansion"
```

#### Test: unterminated dollar-single-quote causes shell syntax error

An unmatched `$'...` sequence is not valid shell syntax. The shell should
reject the script with a diagnostic instead of executing it.

```
begin test "unterminated dollar-single-quote causes shell syntax error"
  script
    printf '%s\n' $'unterminated
  expect
    stdout ""
    stderr "(.|\n)+"
    exit_code !=0
end test "unterminated dollar-single-quote causes shell syntax error"
```
