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

#### Test: long input line is accepted

The shell reads its input in terms of lines, and input lines can be of
unlimited length. This test feeds a moderately long single command line and
checks that the full token is preserved.

```
begin test "long input line is accepted"
  script
    i=0
    payload=
    while [ "$i" -lt 5000 ]; do
      payload="${payload}x"
      i=$((i + 1))
    done
    printf "printf '%%s\\n' %s\n" "$payload" > long_line.sh
    out=$($SHELL long_line.sh)
    printf '%s\n' "${#out}"
  expect
    stdout "5000"
    stderr ""
    exit_code 0
end test "long input line is accepted"
```

#### Test: end of substitution does not delimit token

Tokens are not delimited by the end of a command substitution. The string `suffix` is appended to the substitution to form a single token.

```
begin test "end of substitution does not delimit token"
  script
    printf '%s\n' "$(printf '%s' hello)suffix"
  expect
    stdout "hellosuffix"
    stderr ""
    exit_code 0
end test "end of substitution does not delimit token"
```

#### Test: complete_command executed before next is tokenized

When parsing a program, the shell executes one complete command before
tokenizing and parsing the next. This makes an alias defined by the first
complete command visible to the second one.

```
begin test "complete_command executed before next is tokenized"
  script
    alias after_first="printf '%s\n' tokenized-later"
    after_first
  expect
    stdout "tokenized-later"
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

During token recognition, substitutions are not performed immediately. A token
containing an invalid expansion is still recognized as one word, and the shell
reports the error only when expansion is attempted.

```
begin test "bad expansion parameter error"
  script
    printf '%s\n' ${/}
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

#### Test: blank input is discarded

If token recognition reaches end-of-input without accumulating any token
characters, nothing is executed.

```
begin test "blank input is discarded"
  script
    printf '%s' ' ' | $SHELL
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "blank input is discarded"
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

#### Test: << forms io_here operator without blanks

The `<<` operator is recognized as a single `io_here` token even when it is
adjacent to the command name and delimiter with no surrounding blanks.

```
begin test "<< forms io_here operator without blanks"
  script
    cat<<EOF
    compact heredoc
    EOF
  expect
    stdout "compact heredoc"
    stderr ""
    exit_code 0
end test "<< forms io_here operator without blanks"
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

#### Test: tokens after io_here are saved until body is parsed

Tokens recognized after an `io_here` on the same line are saved and processed
only after the here-document body has been parsed.

```
begin test "tokens after io_here are saved until body is parsed"
  script
    cat <<EOF >saved_after_heredoc.txt
    saved body
    EOF
    cat saved_after_heredoc.txt
  expect
    stdout "saved body"
    stderr ""
    exit_code 0
end test "tokens after io_here are saved until body is parsed"
```

#### Test: backslash quoting of special characters

A backslash begins quoted text during token recognition, preventing operator
characters like `|`, `&`, and `;` from delimiting tokens.

```
begin test "backslash quoting of special characters"
  script
    printf '%s\n' \| \& \;
  expect
    stdout "\|\n&\n;"
    stderr ""
    exit_code 0
end test "backslash quoting of special characters"
```

#### Test: backslash preserves literal value of following character

A backslash keeps `*` in the current word instead of letting it participate in
pathname expansion or token delimiting.

```
begin test "backslash preserves literal value of following character"
  script
    touch a_test_b
    printf '%s\n' a\*b
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
    printf '%s\n' foo\;bar
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
    printf '%s\n' \$foo
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

#### Test: single-quoted text does not delimit surrounding token

Single-quoted text is included unmodified in the current token during token
recognition, and the closing quote does not delimit the token.

```
begin test "single-quoted text does not delimit surrounding token"
  script
    set -- pre'$foo *'post
    printf '%s\n%s\n' "$#" "$1"
  expect
    stdout "1\npre\$foo \*post"
    stderr ""
    exit_code 0
end test "single-quoted text does not delimit surrounding token"
```

#### Test: double-quoted text does not delimit surrounding token

Double-quoted text remains part of the current token, so unquoted characters
immediately before and after it are concatenated into one word.

```
begin test "double-quoted text does not delimit surrounding token"
  script
    set -- pre"a*b"post
    printf '%s\n%s\n' "$#" "$1"
  expect
    stdout "1\nprea\*bpost"
    stderr ""
    exit_code 0
end test "double-quoted text does not delimit surrounding token"
```

#### Test: escaped backslash in double quotes stays in one token

Inside double quotes, `\\` contributes a literal backslash to the current token
without delimiting it.

```
begin test "escaped backslash in double quotes stays in one token"
  script
    set -- pre"\\"post
    printf '%s\n%s\n' "$#" "$1"
  expect
    stdout "1\npre\\post"
    stderr ""
    exit_code 0
end test "escaped backslash in double quotes stays in one token"
```

#### Test: quoted substitutions do not delimit token

Inside double quotes, parameter, command, and arithmetic substitutions are still
recognized, but the quoted field remains part of one token.

```
begin test "quoted substitutions do not delimit token"
  script
    foo=bar
    printf '%s\n' "pre-$foo-$(printf '%s' sub)-$((2+2))-post"
  expect
    stdout "pre-bar-sub-4-post"
    stderr ""
    exit_code 0
end test "quoted substitutions do not delimit token"
```

#### Test: inner double quotes inside command substitution

Double quotes can appear inside a command substitution that is itself within a
double-quoted field, and the entire substitution result remains part of the
surrounding token.

```
begin test "inner double quotes inside command substitution"
  script
    printf '%s\n' pre"$(printf '%s\n' "inner quotes")"post
  expect
    stdout "preinner quotespost"
    stderr ""
    exit_code 0
end test "inner double quotes inside command substitution"
```

#### Test: recursive tokenizing finds matching paren

The shell recursively tokenizes characters to locate the matching closing
parenthesis of a command substitution without delimiting the surrounding token.

```
begin test "recursive tokenizing finds matching paren"
  script
    printf '%s\n' pre"$(printf '%s\n' "(recursive)")"post
  expect
    stdout "pre\(recursive\)post"
    stderr ""
    exit_code 0
end test "recursive tokenizing finds matching paren"
```

#### Test: backquote inside double quotes executes

Inside double quotes, a backquote substitution is recognized without delimiting
the surrounding token.

```
begin test "backquote inside double quotes executes"
  script
    printf '%s\n' "pre`printf '%s\n' sub`post"
  expect
    stdout "presubpost"
    stderr ""
    exit_code 0
end test "backquote inside double quotes executes"
```

#### Test: backslash in double quotes does not delimit token for other chars

Inside double quotes, a backslash before characters other than `$`, `` ` ``,
`"`, `\`, or newline remains part of the current token as a literal backslash.

```
begin test "backslash in double quotes does not delimit token for other chars"
  script
    printf '%s\n' "\n" "\a" "\*"
  expect
    stdout "\\n\n\\a\n\\\*"
    stderr ""
    exit_code 0
end test "backslash in double quotes does not delimit token for other chars"
```

#### Test: quoted expansion result stays in one field

When an expansion occurs within double quotes, the token remains one field
rather than being delimited by blanks or glob characters in the expansion
result.

```
begin test "quoted expansion result stays in one field"
  script
    foo='a b *'
    set -- "pre-$foo-post"
    printf '%s\n%s\n' "$#" "$1"
  expect
    stdout "1\npre-a b \*-post"
    stderr ""
    exit_code 0
end test "quoted expansion result stays in one field"
```

#### Test: parameter expansion candidate does not delimit token

An unquoted `${...}` expansion candidate is recognized as part of the current
word and does not delimit the surrounding token.

```
begin test "parameter expansion candidate does not delimit token"
  script
    foo="a*b"
    printf '%s\n' pre"${foo#a*}"post
  expect
    stdout "pre\*bpost"
    stderr ""
    exit_code 0
end test "parameter expansion candidate does not delimit token"
```

#### Test: nested command substitution inside ${...} stays in one token

The shell reads far enough to find the end of a `${...}` expansion even when the
expansion text contains a nested command substitution.

```
begin test "nested command substitution inside ${...} stays in one token"
  script
    unset foo
    printf '%s\n' pre"${foo:-$(printf '%s' default)}"post
  expect
    stdout "predefaultpost"
    stderr ""
    exit_code 0
end test "nested command substitution inside ${...} stays in one token"
```

#### Test: escaped double quote stays in double-quoted token

A backslash-escaped double quote is included in the current double-quoted token
without terminating it.

```
begin test "escaped double quote stays in double-quoted token"
  script
    set -- pre"\""post
    printf '%s\n%s\n' "$#" "$1"
  expect
    stdout "1\npre""post"
    stderr ""
    exit_code 0
end test "escaped double quote stays in double-quoted token"
```

#### Test: dollar-single-quoted text does not delimit surrounding token

The `$'...'` quoted region is recognized as part of the current token, so
unquoted text immediately before and after it remains in the same word.

```
begin test "dollar-single-quoted text does not delimit surrounding token"
  script
    printf '%s\n' pre$'\x41'post
  expect
    stdout "preApost"
    stderr ""
    exit_code 0
end test "dollar-single-quoted text does not delimit surrounding token"
```

#### Test: backslash-quoted operator characters stay in one token

Backslash quoting keeps operator and quoting characters inside the current word
instead of allowing them to delimit tokens.

```
begin test "backslash-quoted operator characters stay in one token"
  script
    set -- pre\|post pre\&post pre\;post
    printf '%s\n%s\n%s\n%s\n' "$#" "$1" "$2" "$3"
  expect
    stdout "3\npre\|post\npre&post\npre;post"
    stderr ""
    exit_code 0
end test "backslash-quoted operator characters stay in one token"
```

#### Test: single-quoted special characters stay in one token

Single-quoted operator characters and blanks remain inside one token, and the
surrounding unquoted text is concatenated onto the same word.

```
begin test "single-quoted special characters stay in one token"
  script
    set -- pre'| & ; < > ( ) $ ` \ "'post
    printf '%s\n%s\n' "$#" "$1"
  expect
    stdout "1\npre\| & ; < > \( \) \$ ` \\ ""post"
    stderr ""
    exit_code 0
end test "single-quoted special characters stay in one token"
```

#### Test: double-quoted special characters stay in one token

Double-quoted operator characters remain in the current token rather than
delimiting it, and the surrounding unquoted text is concatenated onto the same
word.

```
begin test "double-quoted special characters stay in one token"
  script
    set -- pre"| & ; < > ( )"post
    printf '%s\n%s\n' "$#" "$1"
  expect
    stdout "1\npre\| & ; < > \( \)post"
    stderr ""
    exit_code 0
end test "double-quoted special characters stay in one token"
```

#### Test: quoted blanks do not delimit token

Quoted spaces and tabs are retained within a single token rather than acting as
token delimiters.

```
begin test "quoted blanks do not delimit token"
  script
    set -- "hello world" "a	b"
    printf '%s\n%s\n%s\n' "$#" "$1" "$2"
  expect
    stdout "2\nhello world\na	b"
    stderr ""
    exit_code 0
end test "quoted blanks do not delimit token"
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

#### Test: quoted glob characters stay in one token

Quoted glob characters remain part of the current token, and surrounding
unquoted text is concatenated to the same word.

```
begin test "quoted glob characters stay in one token"
  script
    set -- pre'*'post pre'?'post pre'[abc]'post
    printf '%s\n%s\n%s\n%s\n' "$#" "$1" "$2" "$3"
  expect
    stdout "3\npre\*post\npre\?post\npre\[abc\]post"
    stderr ""
    exit_code 0
end test "quoted glob characters stay in one token"
```

#### Test: quoted conditionally special characters stay in one token

Quoted conditionally special characters remain part of the current token instead
of being interpreted specially.

```
begin test "quoted conditionally special characters stay in one token"
  script
    set -- pre'~'post pre'='post pre'%'post pre'{'post pre'}'post pre','post pre'^'post pre'-'post
    printf '%s\n%s\n%s\n%s\n%s\n%s\n%s\n%s\n%s\n' "$#" "$1" "$2" "$3" "$4" "$5" "$6" "$7" "$8"
  expect
    stdout "8\npre~post\npre=post\npre%post\npre\{post\npre\}post\npre,post\npre\^post\npre-post"
    stderr ""
    exit_code 0
end test "quoted conditionally special characters stay in one token"
```

#### Test: dollar-single-quoted escape stays in one token

An escaped character produced by a dollar-single-quoted region remains part of
the surrounding token.

```
begin test "dollar-single-quoted escape stays in one token"
  script
    printf '%s\n' pre$'\x41\x42'post
  expect
    stdout "preABpost"
    stderr ""
    exit_code 0
end test "dollar-single-quoted escape stays in one token"
```

#### Test: escaped single quote in dollar-single-quotes stays in one token

An escaped single quote inside `$'...'` is included in the current token and
does not terminate the quoted region early.

```
begin test "escaped single quote in dollar-single-quotes stays in one token"
  script
    printf '%s\n' pre$'can\'t'post
  expect
    stdout "precan'tpost"
    stderr ""
    exit_code 0
end test "escaped single quote in dollar-single-quotes stays in one token"
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

Reserved words like `if` are not recognized as such if they contain quoting
characters. In command position, the quoted word is treated as an ordinary
command name.

```
begin test "reserved words not recognized when quoted"
  script
    cat > if <<'EOF'
    #!/bin/sh
    printf '%s\n' quoted-word
    EOF
    chmod +x if
    PATH=".:$PATH" 'if'
  expect
    stdout "quoted-word"
    stderr ""
    exit_code 0
end test "reserved words not recognized when quoted"
```

#### Test: dollar-single-quote variable-length escapes terminate correctly

Variable-length escapes inside `$'...'` terminate correctly, and the resulting
text remains part of the surrounding token.

```
begin test "dollar-single-quote variable-length escapes terminate correctly"
  script
    set -- pre$'\x41'post pre$'\x41Z'post pre$'\101'post pre$'\1012'post
    printf '%s\n%s\n%s\n%s\n%s\n' "$#" "$1" "$2" "$3" "$4"
  expect
    stdout "4\npreApost\npreAZpost\npreApost\npreA2post"
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

The `$'...'` quoted region remains part of the surrounding token even when an
escape produces an embedded newline.

```
begin test "dollar-single-quote basic support"
  script
    printf '%s\n' pre$'hello\nworld'post
  expect
    stdout "prehello\nworldpost"
    stderr ""
    exit_code 0
end test "dollar-single-quote basic support"
```

#### Test: dollar-paren command substitution

The `$(...)` syntax is recognized as one substitution unit and does not delimit
the surrounding token.

```
begin test "dollar-paren command substitution"
  script
    printf '%s\n' pre$(printf '%s' hello)post
  expect
    stdout "prehellopost"
    stderr ""
    exit_code 0
end test "dollar-paren command substitution"
```

#### Test: backtick command substitution

The `` `...` `` syntax is recognized as one substitution unit and does not
delimit the surrounding token.

```
begin test "backtick command substitution"
  script
    printf '%s\n' pre`printf '%s' hello`post
  expect
    stdout "prehellopost"
    stderr ""
    exit_code 0
end test "backtick command substitution"
```

#### Test: nested dollar-paren command substitution

Command substitutions using `$(...)` can nest recursively without delimiting the
surrounding token.

```
begin test "nested dollar-paren command substitution"
  script
    printf '%s\n' pre$(printf '%s' "$(printf '%s' nested)")post
  expect
    stdout "prenestedpost"
    stderr ""
    exit_code 0
end test "nested dollar-paren command substitution"
```

#### Test: arithmetic addition

The `$((...))` arithmetic expansion candidate does not delimit the surrounding
token.

```
begin test "arithmetic addition"
  script
    printf '%s\n' pre$((40 + 2))post
  expect
    stdout "pre42post"
    stderr ""
    exit_code 0
end test "arithmetic addition"
```

#### Test: arithmetic subtraction negative

Arithmetic expansion remains part of the current token even when the result is
negative.

```
begin test "arithmetic subtraction negative"
  script
    printf '%s\n' pre$((3 - 4))post
  expect
    stdout "pre-1post"
    stderr ""
    exit_code 0
end test "arithmetic subtraction negative"
```

#### Test: && forms a single operator token

Two consecutive `&` characters are combined into the `&&` operator token rather
than being treated as separate words, even without surrounding blanks.

```
begin test "&& forms a single operator token"
  script
    true&&printf '%s\n' and-list
  expect
    stdout "and-list"
    stderr ""
    exit_code 0
end test "&& forms a single operator token"
```

#### Test: || forms a single operator token

Two consecutive `|` characters are combined into the `||` operator token rather
than being treated as separate words, even without surrounding blanks.

```
begin test "|| forms a single operator token"
  script
    false||printf '%s\n' or-list
  expect
    stdout "or-list"
    stderr ""
    exit_code 0
end test "|| forms a single operator token"
```

#### Test: single pipe operator delimits words without blanks

An unquoted `|` starts an operator token and delimits the surrounding words even
when there are no blanks around it.

```
begin test "single pipe operator delimits words without blanks"
  script
    printf '%s\n' piped|cat
  expect
    stdout "piped"
    stderr ""
    exit_code 0
end test "single pipe operator delimits words without blanks"
```

#### Test: unquoted > starts redirection operator

An unquoted `>` character starts a redirection operator token and delimits the
preceding word even when there are no surrounding blanks.

```
begin test "unquoted > starts redirection operator"
  script
    printf '%s\n' a>tmp_token.txt
    cat tmp_token.txt
  expect
    stdout "a"
    stderr ""
    exit_code 0
end test "unquoted > starts redirection operator"
```

#### Test: semicolon operator delimits commands without blanks

An unquoted `;` starts an operator token and separates complete commands even
when it appears with no surrounding blanks.

```
begin test "semicolon operator delimits commands without blanks"
  script
    printf '%s\n' first;printf '%s\n' second
  expect
    stdout "first\nsecond"
    stderr ""
    exit_code 0
end test "semicolon operator delimits commands without blanks"
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

#### Test: unquoted tab delimits words

An unquoted tab is a `<blank>` and therefore delimits tokens just like an
unquoted space.

```
begin test "unquoted tab delimits words"
  script
    script=$(printf 'set -- a\tb\nprintf "%%s:%%s:%%s\\n" "$#" "$1" "$2"\n')
    printf '%s' "$script" | $SHELL
  expect
    stdout "2:a:b"
    stderr ""
    exit_code 0
end test "unquoted tab delimits words"
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

#### Test: comment after operator is ignored up to newline

After an operator has delimited the previous token, an unquoted `#` begins a
comment even with no intervening blank.

```
begin test "comment after operator is ignored up to newline"
  script
    printf '%s\n' a;#this is ignored
    printf '%s\n' b
  expect
    stdout "a\nb"
    stderr ""
    exit_code 0
end test "comment after operator is ignored up to newline"
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

#### Test: quoted token is not alias-substituted

A token containing quoting characters is not subject to alias substitution, even
if its unquoted text matches an alias name.

```
begin interactive test "quoted token is not alias-substituted"
  spawn -i
  expect "$ "
  send "cat > foo <<'EOF'\n#!/bin/sh\nprintf '%s\\n' quoted-command\nEOF\nchmod +x foo\nPATH=.:$PATH"
  expect "$ "
  send "alias foo=\"printf '%s\\n' aliased\""
  expect "$ "
  send "'foo'"
  expect "quoted-command"
  sendeof
  wait
end interactive test "quoted token is not alias-substituted"
```

#### Test: alias does not expand in non-command position

Alias substitution is not applied to an ordinary argument token that is not in
command-name position.

```
begin interactive test "alias does not expand in non-command position"
  spawn -i
  expect "$ "
  send "alias foo=\"printf '%s\\n' aliased\""
  expect "$ "
  send "printf '%s\\n' foo"
  expect "foo"
  sendeof
  wait
end interactive test "alias does not expand in non-command position"
```

#### Test: alias name must match the whole token

Alias substitution applies only when the token itself is the alias name. A
longer token that merely starts with the alias name is not substituted.

```
begin interactive test "alias name must match the whole token"
  spawn -i
  expect "$ "
  send "cat > foobar <<'EOF'\n#!/bin/sh\nprintf '%s\\n' whole-token\nEOF\nchmod +x foobar\nPATH=.:$PATH"
  expect "$ "
  send "alias foo=\"printf '%s\\n' aliased\""
  expect "$ "
  send "foobar"
  expect "whole-token"
  sendeof
  wait
end interactive test "alias name must match the whole token"
```

#### Test: token that is not a valid alias name is not substituted

Alias substitution applies only to tokens that are valid alias names. A token
containing a slash is therefore not subject to alias substitution.

```
begin interactive test "token that is not a valid alias name is not substituted"
  spawn -i
  expect "$ "
  send "cat > foo <<'EOF'\n#!/bin/sh\nprintf '%s\\n' slash-token\nEOF\nchmod +x foo"
  expect "$ "
  send "alias foo=\"printf '%s\\n' aliased\""
  expect "$ "
  send "./foo"
  expect "slash-token"
  sendeof
  wait
end interactive test "token that is not a valid alias name is not substituted"
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

#### Test: alias without trailing blank does not chain to next word

If an alias value does not end in a blank, the following token is not checked
for alias substitution merely because it follows that alias expansion.

```
begin interactive test "alias without trailing blank does not chain to next word"
  spawn -i
  expect "$ "
  send "alias a1=\"printf '%s\\\\n'\""
  expect "$ "
  send "alias a2=\"printf '%s\\\\n' chained\""
  expect "$ "
  send "a1 a2"
  expect "a2"
  sendeof
  wait
end interactive test "alias without trailing blank does not chain to next word"
```

#### Test: alias replacement is re-tokenized from the start

After alias substitution, token recognition resumes at the start of the alias
value as if that text had been read from the input.

```
begin interactive test "alias replacement is re-tokenized from the start"
  spawn -i
  expect "$ "
  send "alias make_if=\"if true; then printf '%s\\\\n' retokenized; fi\""
  expect "$ "
  send "make_if"
  expect "retokenized"
  sendeof
  wait
end interactive test "alias replacement is re-tokenized from the start"
```

#### Test: different alias names can recurse

After substituting one alias, token recognition resumes at the start of the
replacement text, allowing a different alias name found there to be substituted
recursively.

```
begin interactive test "different alias names can recurse"
  spawn -i
  expect "$ "
  send "alias a='b'"
  expect "$ "
  send "alias b=\"printf '%s\\n' recursive\""
  expect "$ "
  send "a"
  expect "recursive"
  sendeof
  wait
end interactive test "different alias names can recurse"
```

#### Test: same alias name is not recursively re-expanded

If a token already resulted from substitution of alias `a`, the shell does not
apply alias `a` to that token again at a deeper recursion level.

```
begin interactive test "same alias name is not recursively re-expanded"
  spawn -i
  expect "$ "
  send "alias a=\"a\""
  expect "$ "
  send "a >/dev/null 2>&1 || printf '%s\n' recursion-stopped"
  expect "recursion-stopped"
  sendeof
  wait
end interactive test "same alias name is not recursively re-expanded"
```

#### Test: alias changes do not take effect out of order

When multiple changes are made to the same alias, later changes do not take
effect before earlier ones. This test derives the second definition from the
first, so an out-of-order implementation would not be able to produce the final
value correctly.

```
begin interactive test "alias changes do not take effect out of order"
  spawn -i
  expect "$ "
  send "alias foo=\"printf '%s\\n' first\""
  expect "$ "
  send "alias \"$(alias foo | sed 's/first/second/')\""
  expect "$ "
  send "foo"
  expect "second"
  sendeof
  wait
end interactive test "alias changes do not take effect out of order"
```

#### Test: alias redefinition applies by next complete command

An alias redefinition takes effect no later than the next complete command.

```
begin interactive test "alias redefinition applies by next complete command"
  spawn -i
  expect "$ "
  send "alias foo=\"printf '%s\\n' old\""
  expect "$ "
  send "foo"
  expect "old"
  expect "$ "
  send "alias foo=\"printf '%s\\n' new\""
  expect "$ "
  send "foo"
  expect "new"
  sendeof
  wait
end interactive test "alias redefinition applies by next complete command"
```

#### Test: alias is not inherited by child shell

Alias definitions are not inherited by a separate shell invocation.

```
begin interactive test "alias is not inherited by child shell"
  spawn -i
  expect "$ "
  send "alias outer=\"printf '%s\\n' aliased\""
  expect "$ "
  send "$SHELL -c 'outer' >/dev/null 2>&1 || printf '%s\\n' child-clean"
  expect "child-clean"
  sendeof
  wait
end interactive test "alias is not inherited by child shell"
```

