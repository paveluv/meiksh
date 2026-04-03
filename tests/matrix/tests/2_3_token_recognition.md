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

> Once a token has started, zero or more characters from the input shall be appended to the token until the end of the token is delimited according to one of the rules below.

> When both the start and end of a token have been delimited, the characters forming the token shall be exactly those in the input between the two delimiters, including any quoting characters.

> If the end of input is recognized, the current token (if any) shall be delimited.

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

> In situations where the shell parses its input as a program , once a complete_command has been recognized by the grammar (see 2.10 Shell Grammar ), the complete_command shall be executed before the next complete_command is tokenized and parsed.

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

> The <backslash> and <newline> shall be removed before splitting the input into tokens.

> To specify nesting within the backquoted version, the application shall precede the inner backquotes with <backslash> characters.

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

> During token recognition no substitutions shall be actually performed, and the result token shall contain exactly the characters that appear in the input unmodified, including any embedded or enclosing quotes or substitution operators, between the start and the end of the quoted text.

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

> The shell shall read its input in terms of lines. (For details about how the shell reads its input, see the description of sh .) The input lines can be of unlimited length.

> When it is not processing an io_here , the shell shall break its input into tokens by applying the first applicable rule below to each character in turn in its input.

> At the start of input or after a previous token has just been delimited, the first or next token, respectively, shall start with the first character that has not already been included in a token and is not discarded according to the rules below.

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

> If a rule below indicates that a token is delimited, and no characters have been included in the token, that empty token shall be discarded.

> The shell shall read sufficient input to determine the end of the unit to be expanded (as explained in the cited sections).

> When a TOKEN is seen where one of those annotated productions could be used to reduce the symbol, the applicable rule shall be applied to convert the token identifier type of the TOKEN to a token identifier acceptable at that point in the grammar.

```
begin test "empty token discarded"
  script
    $SHELL -c ';;' 2>/dev/null || true
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "empty token discarded"
```

#### Test: quoted field does not delimit token

> The token shall not be delimited by the end of the quoted field.

> If the previous character was used as part of an operator and the current character is not quoted and can be used with the previous characters to form an operator, it shall be used as part of that (operator) token.

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

> For \

> The token shall not be delimited by the end of the substitution.

> When a TOKEN is subject to alias substitution, the value of the alias shall be processed as if it had been read from the input instead of the TOKEN , with token recognition (see 2.3 Token Recognition ) resuming at the start of the alias value.

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

> When an io_here token has been recognized by the grammar (see 2.10 Shell Grammar ), one or more of the subsequent lines immediately following the next NEWLINE token form the body of a here-document and shall be parsed according to the rules of 2.7.4 Here-Document .

> Any non- NEWLINE tokens (including more io_here tokens) that are recognized while searching for the next NEWLINE token shall be saved for processing after the here-document has been parsed.

> If a saved token is an io_here token, the corresponding here-document shall start on the line immediately following the line containing the trailing delimiter of the previous here-document.

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

> These lines shall be parsed using two major modes: ordinary token recognition and processing of here-documents.

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

> The backquote shall retain its special meaning introducing the other form of command substitution (see 2.6.3 Command Substitution).

> Command substitution shall occur when command(s) are enclosed as follows: $( commands ) or (backquoted version): ` commands ` The shell shall expand the command substitution by executing commands in a subshell environment (see 2.13 Shell Execution Environment ) and replacing the command substitution (the text of the commands string plus the enclosing \

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

> After a token has been categorized as type TOKEN (see 2.10.1 Shell Grammar Lexical Conventions ), including (recursively) any token resulting from an alias substitution, the TOKEN shall be subject to alias substitution if all of the following conditions are true: The TOKEN does not contain any quoting characters.

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

> A <backslash> that is not quoted shall preserve the literal value of the following character, with the exception of a <newline>.

> A sequence of characters starting with a <dollar-sign> immediately followed by a single-quote ( $' ) shall preserve the literal value of all characters up to an unescaped terminating single-quote ( ' ), with the exception of certain <backslash>-escape sequences, as follows: \\\

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

> A <backslash> that is not quoted shall preserve the literal value of the following character, with the exception of a <newline>.

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

> A <backslash> that is not quoted shall preserve the literal value of the following character, with the exception of a <newline>.

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

> If the current character is an unquoted <backslash>, single-quote, or double-quote or is the first character of an unquoted <dollar-sign> single-quote sequence, it shall affect quoting for subsequent characters up to the end of the quoted text.

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

> The sh utility is a command language interpreter that shall execute commands read from a command line string, the standard input, or a specified file.

> If a <newline> immediately follows the <backslash>, the shell shall interpret this as line continuation.

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

> If a <newline> immediately follows the <backslash>, the shell shall interpret this as line continuation.

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

> If a <newline> immediately follows the <backslash>, the shell shall interpret this as line continuation.

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

> Enclosing characters in single-quotes ( '' ) shall preserve the literal value of each character within the single-quotes.

> These <backslash>-escape sequences shall be processed (replaced with the bytes or characters they yield) immediately prior to word expansion (see 2.6 Word Expansions ) of the word in which the dollar-single-quotes sequence occurs.

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

> The <backslash> character shall follow the same rules as for characters in double-quotes described in this section except that it shall additionally retain its special meaning as an escape character when followed by '}' and this shall prevent the escaped '}' from being considered when determining the matching '}' (using the rule in 2.6.2 Parameter Expansion ).

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

> The application shall ensure that a double-quote that is not within \

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

> If the current character is an unquoted '$' or '`' , the shell shall identify the start of any candidates for parameter expansion ( 2.6.2 Parameter Expansion ), command substitution ( 2.6.3 Command Substitution ), or arithmetic expansion ( 2.6.4 Arithmetic Expansion ) from their introductory unquoted character sequences: '$' or \

> The application shall ensure that the commands to be executed are expressed in the language described in 2. Shell Command Language.

> Enclosing characters in double-quotes ( \

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

> During token recognition no substitutions shall be actually performed, and the result token shall contain exactly the characters that appear in the input unmodified, including any embedded or enclosing quotes or substitution operators, between the start and the end of the quoted text.

> The characters found from the beginning of the substitution to its end, allowing for any recursion necessary to recognize embedded constructs, shall be included unmodified in the result token, including any embedded or enclosing substitution operators or quotes.

> The input characters within the quoted string that are also enclosed between \

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

> The tokenizing rules in 2.3 Token Recognition shall be applied recursively to find the matching ')' .

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

> The backquote and <dollar-sign> characters shall follow the same rules as for characters in double-quotes described in this section.

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

> Enclosing characters in double-quotes ( \

> Outside of \

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

> For parameter expansions other than the four varieties that provide for substring processing, within the string of characters from an enclosed \

> When double-quotes are used to quote a parameter expansion, command substitution, or arithmetic expansion, the literal value of all characters within the result of the expansion shall be preserved.

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

> For the four varieties of parameter expansion that provide for substring processing (see 2.6.2 Parameter Expansion ), within the string of characters from an enclosed \

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

> Enclosing characters in double-quotes ( \

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

> The format for arithmetic expansion shall be as follows: $(( expression )) The expression shall be treated as if it were in double-quotes, except that a double-quote inside the expression is not treated specially.

> Outside of \

> The application shall ensure that a double-quote that is not within \

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

> The application shall quote the following characters if they are to represent themselves: | & ; < > ( ) $ ` \\ \

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

> The application shall quote the following characters if they are to represent themselves: | & ; < > ( ) $ ` \\ \

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

> The application shall quote the following characters if they are to represent themselves: | & ; < > ( ) $ ` \\ \

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

> The application shall quote the following characters if they are to represent themselves: | & ; < > ( ) $ ` \\ \

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

> The application shall quote the following characters if they are to represent themselves: | & ; < > ( ) $ ` \\ \

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

> The application shall quote the following characters if they are to represent themselves: | & ; < > ( ) $ ` \\ \

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

> The application shall quote the following characters if they are to represent themselves: | & ; < > ( ) $ ` \\ \

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

> The application shall quote the following characters if they are to represent themselves: | & ; < > ( ) $ ` \\ \

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

> A sequence of characters starting with a <dollar-sign> immediately followed by a single-quote ( $' ) shall preserve the literal value of all characters up to an unescaped terminating single-quote ( ' ), with the exception of certain <backslash>-escape sequences, as follows: \\\

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

> If a <backslash>-escape sequence represents a single-quote character (for example \\' ), that sequence shall not terminate the dollar-single-quote sequence.

> If the current character is an unquoted <backslash>, single-quote, or double-quote or is the first character of an unquoted <dollar-sign> single-quote sequence, it shall affect quoting for subsequent characters up to the end of the quoted text.

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

> When an io_here token has been recognized by the grammar (see 2.10 Shell Grammar ), one or more of the subsequent lines immediately following the next NEWLINE token form the body of a here-document and shall be parsed according to the rules of 2.7.4 Here-Document .

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

> The following words shall be recognized as reserved words: ! { } case do done elif else esac fi for if in then until while.

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

> In cases where a variable number of characters can be used to specify an escape sequence ( \\x XX and \\ ddd ), the escape sequence shall be terminated by the first character that is not of the expected type or, for \\ ddd sequences, when the maximum number of characters specified has been found, whichever occurs first.

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

> The following words shall be recognized as reserved words: ! { } case do done elif else esac fi for if in then until while.

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

> The following words shall be recognized as reserved words: ! { } case do done elif else esac fi for if in then until while.

> This recognition shall only occur when none of the characters is quoted and when the word is used as: The first word of a command The first word following one of the reserved words other than case , for , or in The third word in a case command (only in is valid in this case) The third word in a for command (only in and do are valid in this case) See the grammar in 2.10 Shell Grammar .

> Once a token is delimited, it is categorized as required by the grammar in 2.10 Shell Grammar.

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

#### Test: alias trailing blank triggers expansion of next word

> An implementation may defer the effect of a change to an alias but the change shall take effect no later than the completion of the currently executing complete_command (see 2.10 Shell Grammar ).

> If the value of the alias replacing the TOKEN ends in a <blank> that would be unquoted after substitution, and optionally if it ends in a <blank> that would be quoted after substitution, the shell shall check the next token in the input, if it is a TOKEN , for alias substitution; this process shall continue until a TOKEN is found that is not a valid alias or an alias value does not end in such a <blank>.

> Either the TOKEN is being considered for alias substitution because it follows an alias substitution whose replacement value ended with a <blank> (see below) or the TOKEN could be parsed as the command name word of a simple command (see 2.10 Shell Grammar), based on this TOKEN and the tokens (if any) that preceded it, but ignoring whether any subsequent characters would allow that.

```
begin test "alias trailing blank triggers expansion of next word"
  script
    alias myalias="echo "
    myalias hello
  expect
    stdout "hello"
    stderr ""
    exit_code 0
end test "alias trailing blank triggers expansion of next word"
```

#### Test: dollar-single-quote basic support

> If the current character is an unquoted <backslash>, single-quote, or double-quote or is the first character of an unquoted <dollar-sign> single-quote sequence, it shall affect quoting for subsequent characters up to the end of the quoted text.

> A sequence of characters starting with a <dollar-sign> immediately followed by a single-quote ( $' ) shall preserve the literal value of all characters up to an unescaped terminating single-quote ( ' ), with the exception of certain <backslash>-escape sequences, as follows: \\\

> These <backslash>-escape sequences shall be processed (replaced with the bytes or characters they yield) immediately prior to word expansion (see 2.6 Word Expansions ) of the word in which the dollar-single-quotes sequence occurs.

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

> Enclosing characters in double-quotes ( \

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

> During token recognition no substitutions shall be actually performed, and the result token shall contain exactly the characters that appear in the input unmodified, including any embedded or enclosing quotes or substitution operators, between the start and the end of the quoted text.

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

> While processing the characters, if instances of expansions or quoting are found nested within the substitution, the shell shall recursively process them in the manner specified for the construct that is found.

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

> The arithmetic expression shall be processed according to the rules given in 1.1.2.1 Arithmetic Precision and Operations , with the following exceptions: Only signed long integer arithmetic is required.

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

> The expansions that are performed for a given word shall be performed in the following order: Tilde expansion (see 2.6.1 Tilde Expansion ), parameter expansion (see 2.6.2 Parameter Expansion ), command substitution (see 2.6.3 Command Substitution ), and arithmetic expansion (see 2.6.4 Arithmetic Expansion ) shall be performed, beginning to end.

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

> If the current character is not quoted and can be used as the first character of a new operator, the current token (if any) shall be delimited.

> The current character shall be used as the beginning of the next (operator) token.

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

> If the previous character was used as part of an operator and the current character is not quoted and can be used with the previous characters to form an operator, it shall be used as part of that (operator) token.

> If the previous character was used as part of an operator and the current character cannot be used with the previous characters to form an operator, the operator containing the previous character shall be delimited.

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

> If the current character is an unquoted <blank>, any token containing the previous character is delimited and the current character shall be discarded.

> If the previous character was part of a word, the current character shall be appended to that word.

> The current character is used as the start of a new word.

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

> If the current character is a '#' , it and all subsequent characters up to, but excluding, the next <newline> shall be discarded as a comment.

> The <newline> that ends the line is not considered part of the comment.

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

> The application shall quote the following characters if they are to represent themselves: | & ; < > ( ) $ ` \\ \

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

> If one of these conditions is true, the initial fields shall be retained as separate fields, except that if the parameter being expanded was embedded within a word, the first field shall be joined with the beginning part of the original word and the last field shall be joined with the end part of the original word.

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

> When a TOKEN is subject to alias substitution, the value of the alias shall be processed as if it had been read from the input instead of the TOKEN , with token recognition (see 2.3 Token Recognition ) resuming at the start of the alias value.

> An implementation may defer the effect of a change to an alias but the change shall take effect no later than the completion of the currently executing complete_command (see 2.10 Shell Grammar ).

> Changes to aliases shall not take effect out of order.

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

> If the value of the alias replacing the TOKEN ends in a <blank> that would be unquoted after substitution, and optionally if it ends in a <blank> that would be quoted after substitution, the shell shall check the next token in the input, if it is a TOKEN , for alias substitution; this process shall continue until a TOKEN is found that is not a valid alias or an alias value does not end in such a <blank>.

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

