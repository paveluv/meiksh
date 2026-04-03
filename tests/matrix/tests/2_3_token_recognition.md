# Test Suite for 2.3 Token Recognition

This test suite verifies the POSIX shell token recognition rules, including character processing, quoting interaction with tokenization, and alias substitution.

## Table of contents

- [2.3 Token Recognition](#23-token-recognition)
- [2.3.1 Alias Substitution](#231-alias-substitution)

## 2.3 Token Recognition

The shell shall read its input in terms of lines. (For details about how the shell reads its input, see the description of [*sh*](../utilities/sh.md#).) The input lines can be of unlimited length. These lines shall be parsed using two major modes: ordinary token recognition and processing of here-documents.

When an **io_here** token has been recognized by the grammar (see [2.10 Shell Grammar](#210-shell-grammar)), one or more of the subsequent lines immediately following the next **NEWLINE** token form the body of a here-document and shall be parsed according to the rules of [2.7.4 Here-Document](#274-here-document). Any non-**NEWLINE** tokens (including more **io_here** tokens) that are recognized while searching for the next **NEWLINE** token shall be saved for processing after the here-document has been parsed. If a saved token is an **io_here** token, the corresponding here-document shall start on the line immediately following the line containing the trailing delimiter of the previous here-document. If any saved token includes a `<newline>` character, the behavior is unspecified.

When it is not processing an **io_here**, the shell shall break its input into tokens by applying the first applicable rule below to each character in turn in its input. At the start of input or after a previous token has just been delimited, the first or next token, respectively, shall start with the first character that has not already been included in a token and is not discarded according to the rules below. Once a token has started, zero or more characters from the input shall be appended to the token until the end of the token is delimited according to one of the rules below. When both the start and end of a token have been delimited, the characters forming the token shall be exactly those in the input between the two delimiters, including any quoting characters. If a rule below indicates that a token is delimited, and no characters have been included in the token, that empty token shall be discarded.

1. If the end of input is recognized, the current token (if any) shall be delimited.
2. If the previous character was used as part of an operator and the current character is not quoted and can be used with the previous characters to form an operator, it shall be used as part of that (operator) token.
3. If the previous character was used as part of an operator and the current character cannot be used with the previous characters to form an operator, the operator containing the previous character shall be delimited.
4. If the current character is an unquoted `<backslash>`, single-quote, or double-quote or is the first character of an unquoted `<dollar-sign>` single-quote sequence, it shall affect quoting for subsequent characters up to the end of the quoted text. The rules for quoting are as described in [2.2 Quoting](#22-quoting). During token recognition no substitutions shall be actually performed, and the result token shall contain exactly the characters that appear in the input unmodified, including any embedded or enclosing quotes or substitution operators, between the start and the end of the quoted text. The token shall not be delimited by the end of the quoted field.
5. If the current character is an unquoted `'$'` or ``'`'``, the shell shall identify the start of any candidates for parameter expansion ( [2.6.2 Parameter Expansion](#262-parameter-expansion)), command substitution ( [2.6.3 Command Substitution](#263-command-substitution)), or arithmetic expansion ( [2.6.4 Arithmetic Expansion](#264-arithmetic-expansion)) from their introductory unquoted character sequences: `'$'` or `"${"`, `"$("` or ``'`'``, and `"$(("`, respectively. The shell shall read sufficient input to determine the end of the unit to be expanded (as explained in the cited sections). While processing the characters, if instances of expansions or quoting are found nested within the substitution, the shell shall recursively process them in the manner specified for the construct that is found. For `"$("` and ``'`'`` only, if instances of **io_here** tokens are found nested within the substitution, they shall be parsed according to the rules of [2.7.4 Here-Document](#274-here-document); if the terminating `')'` or ``'`'`` of the substitution occurs before the **NEWLINE** token marking the start of the here-document, the behavior is unspecified. The characters found from the beginning of the substitution to its end, allowing for any recursion necessary to recognize embedded constructs, shall be included unmodified in the result token, including any embedded or enclosing substitution operators or quotes. The token shall not be delimited by the end of the substitution.
6. If the current character is not quoted and can be used as the first character of a new operator, the current token (if any) shall be delimited. The current character shall be used as the beginning of the next (operator) token.
7. If the current character is an unquoted `<blank>`, any token containing the previous character is delimited and the current character shall be discarded.
8. If the previous character was part of a word, the current character shall be appended to that word.
9. If the current character is a `'#'`, it and all subsequent characters up to, but excluding, the next `<newline>` shall be discarded as a comment. The `<newline>` that ends the line is not considered part of the comment.
10. The current character is used as the start of a new word.

Once a token is delimited, it is categorized as required by the grammar in [2.10 Shell Grammar](#210-shell-grammar).

In situations where the shell parses its input as a *program*, once a *complete_command* has been recognized by the grammar (see [2.10 Shell Grammar](#210-shell-grammar)), the *complete_command* shall be executed before the next *complete_command* is tokenized and parsed.

### Tests

#### Test: end of substitution does not delimit token

Tokens are not delimited by the end of a command substitution. The string `suffix` is appended to the substitution to form a single token.

```
begin test "end of substitution does not delimit token"
  script
    echo $(echo hello)suffix
  expect
    stdout "hellosuffix"
    stderr ""
    exit_code 0
end test "end of substitution does not delimit token"
```

#### Test: complete_command executed before next is tokenized

When parsing an interactive shell or a script, the shell executes a complete command before tokenizing and parsing the next one. This means variables set in a command take effect immediately.

```
begin test "complete_command executed before next is tokenized"
  script
    x=first
    echo $x
    x=second
    echo $x
  expect
    stdout "first\nsecond"
    stderr ""
    exit_code 0
end test "complete_command executed before next is tokenized"
```

#### Test: line continuation: backslash-newline removed before tokenizing

A backslash followed by a newline is interpreted as line continuation and removed entirely before the shell splits the input into tokens, meaning it does not act as a token separator.

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

#### Test: bad expansion parameter error

During token recognition, substitutions aren't actually performed. Even if a token contains a substitution that will eventually cause a syntax or expansion error, the shell parses it as a word first and errors out during the expansion phase.

```
begin test "bad expansion parameter error"
  script
    echo ${/} 2>/dev/null
  expect
    stdout ""
    stderr ".+"
    exit_code !=0
end test "bad expansion parameter error"
```

#### Test: end of input delimits current token

The end of input (EOF) automatically delimits the current token being parsed without needing a trailing newline or blank.

```
begin test "end of input delimits current token"
  script
    printf '%s' 'echo lastword' | $SHELL
  expect
    stdout "lastword"
    stderr ""
    exit_code 0
end test "end of input delimits current token"
```

#### Test: empty token discarded

If a token is delimited but hasn't accumulated any characters (such as between two semicolons `;;`), that empty token is discarded and not treated as a word.

```
begin test "empty token discarded"
  script
    printf '%s' ' ' | $SHELL
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "empty token discarded"
```

#### Test: quoted field does not delimit token

A quoted field does not delimit a token. Unquoted text appended immediately after a quoted field (or vice versa) is treated as a single token.

```
begin test "quoted field does not delimit token"
  script
    echo "hello"world
  expect
    stdout "helloworld"
    stderr ""
    exit_code 0
end test "quoted field does not delimit token"
```

#### Test: substitution does not delimit token nested

A command substitution does not delimit a token. Any preceding or trailing text is concatenated with the substitution to form a single token.

```
begin test "substitution does not delimit token nested"
  script
    echo prefix_$(echo inner)_suffix
  expect
    stdout "prefix_inner_suffix"
    stderr ""
    exit_code 0
end test "substitution does not delimit token nested"
```

#### Test: here-document body after io_here

When a here-document redirection (`<<EOF`) is recognized, the shell reads the body of the here-document starting from the line immediately following the next newline character.

```
begin test "here-document body after io_here"
  script
    cat <<EOF
    hello from heredoc
    EOF
  expect
    stdout "hello from heredoc"
    stderr ""
    exit_code 0
end test "here-document body after io_here"
```

#### Test: multiple here-documents on same line

If multiple here-document redirections appear on the same line, their bodies are read sequentially in the order they were defined, starting from the following lines.

```
begin test "multiple here-documents on same line"
  script
    cat <<A; cat <<B
    first
    A
    second
    B
  expect
    stdout "first\nsecond"
    stderr ""
    exit_code 0
end test "multiple here-documents on same line"
```

#### Test: here-doc nested in command substitution

A here-document can be nested safely inside a command substitution, provided the termination of the substitution occurs after the termination of the here-document.

```
begin test "here-doc nested in command substitution"
  script
    echo $(cat <<EOF
    nested_heredoc
    EOF
    )
  expect
    stdout "nested_heredoc"
    stderr ""
    exit_code 0
end test "here-doc nested in command substitution"
```

#### Test: backslash quoting of special characters

A backslash preserves the literal value of the following character, preventing special characters like `|`, `&`, and `;` from acting as operators.

```
begin test "backslash quoting of special characters"
  script
    echo \| \& \;
  expect
    stdout "\| & ;"
    stderr ""
    exit_code 0
end test "backslash quoting of special characters"
```

#### Test: backslash preserves literal value of following character

A backslash escapes the literal value of a wildcard character like `*`, preventing pathname expansion.

```
begin test "backslash preserves literal value of following character"
  script
    touch a_test_b
    echo a\*b
  expect
    stdout "a\*b"
    stderr ""
    exit_code 0
end test "backslash preserves literal value of following character"
```

#### Test: backslash escapes semicolon so it is literal

A backslash before a semicolon prevents the shell from interpreting it as a command delimiter, passing it as a literal character in the argument.

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

A backslash before a space prevents it from acting as a token separator, keeping the two words joined as a single argument.

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

#### Test: backslash preserves dollar sign literally

A backslash escapes a dollar sign, preventing it from initiating parameter expansion and preserving it literally.

```
begin test "backslash preserves dollar sign literally"
  script
    echo \$foo
  expect
    stdout "\$foo"
    stderr ""
    exit_code 0
end test "backslash preserves dollar sign literally"
```

#### Test: backslash-newline is line continuation

A backslash immediately followed by a newline acts as line continuation, allowing a single token (like a command name) to span multiple lines.

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

Line continuation can occur between tokens, acting as a simple continuation rather than a token separator.

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

Multiple consecutive backslash-newline sequences are properly processed as line continuations, joining the lines.

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

#### Test: single quotes preserve all characters literally

Enclosing characters in single quotes preserves the literal value of all characters within, including wildcards and dollar signs.

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

#### Test: double quotes prevent wildcard expansion

Enclosing characters in double quotes preserves their literal value and prevents wildcard expansion (like `*`).

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

Inside double quotes, a backslash retains its special meaning only before certain characters (like `$`, `\``, `"`). A backslash preceding another backslash escapes it, producing a single literal backslash.

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

#### Test: double quotes allow parameter and command and arithmetic expansion

Inside double quotes, the dollar sign (`$`) and backquote retain their special meaning, allowing parameter, command, and arithmetic expansions to occur.

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

Double quotes can be nested inside a command substitution that is itself double-quoted. The inner quotes define the command to be executed.

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

The shell recursively tokenizes characters to correctly locate the matching closing parenthesis of a command substitution.

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

Inside double quotes, the backquote retains its special meaning and introduces a command substitution.

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

Inside double quotes, a backslash only acts as an escape character when followed by `$`, `\``, `"`, `\`, or a newline. Otherwise, it is treated as a literal backslash.

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

When an expansion occurs within double quotes, the expanded value is not subjected to field splitting or pathname expansion, preserving characters like spaces and wildcards literally.

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

Inside double-quotes, substring processing operations (like `${var#pattern}`) are unaffected by the outer double-quotes, allowing patterns to be processed normally.

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

Within a parameter expansion (like `${...}`), the double-quotes preserve the literal value of all characters except `"`, `\``, `$`, and `\`.

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

#### Test: escaped double quote inside double quotes

A double quote can be included literally inside a double-quoted string if it is preceded by a backslash.

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

#### Test: dollar-single-quote newline escape

The `$'...'` quoting mechanism processes backslash escapes such as `\n` to produce a literal newline.

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

#### Test: backslash-quoting preserves literal special characters

Backslash quoting preserves the literal value of special characters like `<` and `>`.

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

Single quotes preserve the literal value of all enclosed special characters.

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

Double quotes preserve the literal value of special characters like `|`, `;`, and `<`.

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

Quoting spaces and tabs prevents them from acting as token separators, keeping the text as a single argument.

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

A backslash-newline pair is removed entirely and does not produce a literal newline in the output.

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

Quoting prevents wildcard characters from triggering pathname expansion.

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

Quoting preserves characters that might be conditionally special under certain circumstances.

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

#### Test: dollar-single-quote hex escape

The `$'...'` mechanism processes hex escapes (like `\x41`) into literal characters.

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

Inside `$'...'`, a single quote can be escaped with a backslash to include it literally.

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

#### Test: newline delimits token

A newline character delimits the current token and acts as a command separator.

```
begin test "newline delimits token"
  script
    echo a
    echo b
  expect
    stdout "a\nb"
    stderr ""
    exit_code 0
end test "newline delimits token"
```

#### Test: reserved words not recognized when quoted

Reserved words like `if` are not recognized as such if they contain any quoting characters.

```
begin test "reserved words not recognized when quoted"
  script
    echo "if"
  expect
    stdout "if"
    stderr ""
    exit_code 0
end test "reserved words not recognized when quoted"
```

#### Test: dollar-single-quote variable-length escapes terminate correctly

Octal and hex escapes in `$'...'` terminate correctly when the escape sequence ends.

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

#### Test: reserved words work in correct positions

Reserved words are correctly recognized when unquoted and occurring in positions where they are expected (like the beginning of a command).

```
begin test "reserved words work in correct positions"
  script
    if true; then
      echo yes
    fi
  expect
    stdout "yes"
    stderr ""
    exit_code 0
end test "reserved words work in correct positions"
```

#### Test: case/esac reserved words

The `case` and `esac` reserved words delimit the switch statement correctly.

```
begin test "case/esac reserved words"
  script
    case x in x) echo match ;; esac
  expect
    stdout "match"
    stderr ""
    exit_code 0
end test "case/esac reserved words"
```

#### Test: dollar-single-quote basic support

The `$'...'` quoting mechanism correctly interprets the enclosed string and its escapes.

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

#### Test: dollar-paren command substitution

The `$(...)` syntax correctly performs command substitution.

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

#### Test: backtick command substitution

The `` `...` `` syntax correctly performs command substitution.

```
begin test "backtick command substitution"
  script
    echo `echo hello`
  expect
    stdout "hello"
    stderr ""
    exit_code 0
end test "backtick command substitution"
```

#### Test: nested dollar-paren command substitution

Command substitutions using `$(...)` can be safely nested.

```
begin test "nested dollar-paren command substitution"
  script
    echo $(echo $(echo nested))
  expect
    stdout "nested"
    stderr ""
    exit_code 0
end test "nested dollar-paren command substitution"
```

#### Test: arithmetic addition

The `$((...))` syntax evaluates arithmetic expressions like addition.

```
begin test "arithmetic addition"
  script
    echo $((40 + 2))
  expect
    stdout "42"
    stderr ""
    exit_code 0
end test "arithmetic addition"
```

#### Test: arithmetic subtraction negative

The `$((...))` syntax evaluates arithmetic expressions like subtraction, properly yielding negative numbers.

```
begin test "arithmetic subtraction negative"
  script
    echo $((3 - 4))
  expect
    stdout "-1"
    stderr ""
    exit_code 0
end test "arithmetic subtraction negative"
```

#### Test: unquoted > is a control operator

An unquoted `>` character acts as a redirection control operator and delimits the preceding token.

```
begin test "unquoted > is a control operator"
  script
    echo a>tmp_token.txt
    cat tmp_token.txt
  expect
    stdout "a"
    stderr ""
    exit_code 0
end test "unquoted > is a control operator"
```

#### Test: >> forms a single append operator

Consecutive unquoted `>` characters form a single `>>` append operator.

```
begin test ">> forms a single append operator"
  script
    echo a >tmp_token.txt
    echo b >>tmp_token.txt
    cat tmp_token.txt
  expect
    stdout "a\nb"
    stderr ""
    exit_code 0
end test ">> forms a single append operator"
```

#### Test: multiple blanks between words

Unquoted blanks act as token delimiters; multiple consecutive blanks simply delimit the tokens and are discarded.

```
begin test "multiple blanks between words"
  script
    echo a      b
  expect
    stdout "a b"
    stderr ""
    exit_code 0
end test "multiple blanks between words"
```

#### Test: comments ignored up to newline

An unquoted `#` introduces a comment, causing the shell to ignore all subsequent characters up to (but not including) the next newline.

```
begin test "comments ignored up to newline"
  script
    echo a # this is a comment
    echo b
  expect
    stdout "a\nb"
    stderr ""
    exit_code 0
end test "comments ignored up to newline"
```

#### Test: quoted # is not a comment

A `#` character enclosed in quotes does not introduce a comment and is treated literally.

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

#### Test: # in middle of word is not a comment

A `#` character in the middle of a word does not introduce a comment; it is treated as part of the word.

```
begin test "# in middle of word is not a comment"
  script
    echo a#b
  expect
    stdout "a#b"
    stderr ""
    exit_code 0
end test "# in middle of word is not a comment"
```

## 2.3.1 Alias Substitution

After a token has been categorized as type **TOKEN** (see [2.10.1 Shell Grammar Lexical Conventions](#2101-shell-grammar-lexical-conventions)), including (recursively) any token resulting from an alias substitution, the **TOKEN** shall be subject to alias substitution if all of the following conditions are true:

- The **TOKEN** does not contain any quoting characters.
- The **TOKEN** is a valid alias name (see XBD [*3.10 Alias Name*](../basedefs/V1_chap03.md#310-alias-name)).
- An alias with that name is in effect.
- The **TOKEN** did not either fully or, optionally, partially result from an alias substitution of the same alias name at any earlier recursion level.
- Either the **TOKEN** is being considered for alias substitution because it follows an alias substitution whose replacement value ended with a `<blank>` (see below) or the **TOKEN** could be parsed as the command name word of a simple command (see [2.10 Shell Grammar](#210-shell-grammar)), based on this **TOKEN** and the tokens (if any) that preceded it, but ignoring whether any subsequent characters would allow that.

except that if the **TOKEN** meets the above conditions and would be recognized as a reserved word (see [2.4 Reserved Words](#24-reserved-words)) if it occurred in an appropriate place in the input, it is unspecified whether the **TOKEN** is subject to alias substitution.

When a **TOKEN** is subject to alias substitution, the value of the alias shall be processed as if it had been read from the input instead of the **TOKEN**, with token recognition (see [2.3 Token Recognition](#23-token-recognition)) resuming at the start of the alias value. When the end of the alias value is reached, the shell may behave as if an additional `<space>` character had been read from the input after the **TOKEN** that was replaced. If it does not add this `<space>`, it is unspecified whether the current token is delimited before token recognition is applied to the character (if any) that followed the **TOKEN** in the input.

**Note:** A future version of this standard may disallow adding this `<space>`.

If the value of the alias replacing the **TOKEN** ends in a `<blank>` that would be unquoted after substitution, and optionally if it ends in a `<blank>` that would be quoted after substitution, the shell shall check the next token in the input, if it is a **TOKEN**, for alias substitution; this process shall continue until a **TOKEN** is found that is not a valid alias or an alias value does not end in such a `<blank>`.

An implementation may defer the effect of a change to an alias but the change shall take effect no later than the completion of the currently executing *complete_command* (see [2.10 Shell Grammar](#210-shell-grammar)). Changes to aliases shall not take effect out of order. Implementations may provide predefined aliases that are in effect when the shell is invoked.

When used as specified by this volume of POSIX.1-2024, alias definitions shall not be inherited by separate invocations of the shell or by the utility execution environments invoked by the shell; see [2.13 Shell Execution Environment](#213-shell-execution-environment) .

### Tests

#### Test: alias substitution

A token that forms a valid alias name is replaced by its alias definition, and the shell executes the substituted text.

```
begin interactive test "alias substitution"
  spawn -i
  expect "$ "
  send "alias foo=\"echo aliased\""
  expect "$ "
  send "foo"
  expect "aliased"
  sendeof
  wait
end interactive test "alias substitution"
```

#### Test: alias with trailing space chains to next word

If an alias value ends with a space, the next word is also evaluated for alias substitution.

```
begin interactive test "alias with trailing space chains to next word"
  spawn -i
  expect "$ "
  send "alias a1=\"echo \""
  expect "$ "
  send "alias a2=\"chained\""
  expect "$ "
  send "a1 a2"
  expect "chained"
  sendeof
  wait
end interactive test "alias with trailing space chains to next word"
```

