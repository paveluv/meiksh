# Test Suite for 2.10 Shell Grammar

This test suite covers **Section 2.10 Shell Grammar** of the POSIX.1-2024
Shell Command Language specification, including lexical conventions and
grammar rules.

## Table of contents

- [2.10 Shell Grammar](#210-shell-grammar)
- [2.10.1 Shell Grammar Lexical Conventions](#2101-shell-grammar-lexical-conventions)
- [2.10.2 Shell Grammar Rules](#2102-shell-grammar-rules)

## 2.10 Shell Grammar

The following grammar defines the Shell Command Language. This formal syntax shall take precedence over the preceding text syntax description.

### Tests

#### Test: variable assignment and expansion in simple command

The formal grammar takes precedence over textual descriptions. This test
exercises a basic simple command with variable assignment and expansion,
confirming the grammar handles it correctly.

```
begin test "variable assignment and expansion in simple command"
  script
    var="value"
    echo "$var"
  expect
    stdout "value"
    stderr ""
    exit_code 0
end test "variable assignment and expansion in simple command"
```

## 2.10.1 Shell Grammar Lexical Conventions

The input language to the shell shall be first recognized at the character level. The resulting tokens shall be classified by their immediate context according to the following rules (applied in order). These rules shall be used to determine what a "token" is that is subject to parsing at the token level. The rules for token recognition in [2.3 Token Recognition](#23-token-recognition) shall apply.

1. If the token is an operator, the token identifier for that operator shall result.
2. If the string consists solely of digits and the delimiter character is one of `'<'` or `'>'`, the token identifier **IO_NUMBER** shall result.
3. If the string contains at least three characters, begins with a `<left-curly-bracket>` (`'{'`) and ends with a `<right-curly-bracket>` (`'}'`), and the delimiter character is one of `'<'` or `'>'`, the token identifier **IO_LOCATION** may result; if the result is not **IO_LOCATION**, the token identifier **TOKEN** shall result.
4. Otherwise, the token identifier **TOKEN** shall result.

Further distinction on **TOKEN** is context-dependent. It may be that the same **TOKEN** yields **WORD**, a **NAME**, an **ASSIGNMENT_WORD**, or one of the reserved words below, dependent upon the context. Some of the productions in the grammar below are annotated with a rule number from the following list. When a **TOKEN** is seen where one of those annotated productions could be used to reduce the symbol, the applicable rule shall be applied to convert the token identifier type of the **TOKEN** to:

- The token identifier of the recognized reserved word, for rule 1
- A token identifier acceptable at that point in the grammar, for all other rules

The reduction shall then proceed based upon the token identifier type yielded by the rule applied. When more than one rule applies, the highest numbered rule shall apply (which in turn may refer to another rule). (Note that except in rule 7, the presence of an `'='` in the token has no effect.)

The **WORD** tokens shall have the word expansion rules applied to them immediately before the associated command is executed, not at the time the command is parsed.

### Tests

#### Test: digit before redirection parsed as IO_NUMBER

When a string consists solely of digits and is immediately followed by
`<` or `>`, the token is classified as IO_NUMBER, directing the
redirection to the specified file descriptor.

```
begin test "digit before redirection parsed as IO_NUMBER"
  script
    echo content > tmp_grammar.txt
    0<tmp_grammar.txt
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "digit before redirection parsed as IO_NUMBER"
```

#### Test: space before redirection not parsed as IO_NUMBER

A space between the digit and the redirection operator prevents the digit
from being classified as IO_NUMBER; `0` is treated as a command name instead.

```
begin test "space before redirection not parsed as IO_NUMBER"
  script
    echo content > tmp_grammar.txt
    0 <tmp_grammar.txt
  expect
    stdout ""
    stderr ".+"
    exit_code !=0
end test "space before redirection not parsed as IO_NUMBER"
```

#### Test: quoted here-document delimiter is quote-removed

Rule 3 applies quote removal to the here-document delimiter word to determine
the delimiter text. A quoted delimiter still ends on the unquoted delimiter
line, and expansions inside the body are suppressed.

```
begin test "quoted here-document delimiter is quote-removed"
  script
    cat << \EOF
    $var
    EOF
  expect
    stdout "\$var"
    stderr ""
    exit_code 0
end test "quoted here-document delimiter is quote-removed"
```

#### Test: double-quoted here-document delimiter suppresses expansion

Rule 3 applies quote removal to the delimiter word. When the delimiter
is double-quoted (`"EOF"`), the quotes are removed to determine the
actual delimiter text, and parameter expansion inside the here-document
body is suppressed — the same behavior as backslash-quoting.

```
begin test "double-quoted here-document delimiter suppresses expansion"
  script
    var=hello
    cat <<"EOF"
    $var
    EOF
  expect
    stdout "\$var"
    stderr ""
    exit_code 0
end test "double-quoted here-document delimiter suppresses expansion"
```

#### Test: assignment word in command prefix

A TOKEN containing an unquoted `=` with a valid name prefix is classified as
ASSIGNMENT_WORD in command prefix position (rule 7). The highest numbered
applicable rule is used.

```
begin test "assignment word in command prefix"
  script
    var=123 env | grep -q "^var=123" && echo "assignment"
  expect
    stdout "assignment"
    stderr ""
    exit_code 0
end test "assignment word in command prefix"
```

## 2.10.2 Shell Grammar Rules

1. [Command Name] When the **TOKEN** is exactly a reserved word, the token identifier for that reserved word shall result. Otherwise, the token **WORD** shall be returned. Also, if the parser is in any state where only a reserved word could be the next correct token, proceed as above. Rule 1 is not directly referenced in the grammar, but is referred to by other rules, or applies globally.
  **Note:** Because at this point quoting characters (`<backslash>`, single-quote, `<quotation-mark>`, and the `<dollar-sign>` single-quote sequence) are retained in the token, quoted strings cannot be recognized as reserved words. This rule also implies that reserved words are not recognized except in certain positions in the input, such as after a `<newline>` or `<semicolon>`; the grammar presumes that if the reserved word is intended, it is properly delimited by the user, and does not attempt to reflect that requirement directly. Also note that line joining is done before tokenization, as described in [2.2.1 Escape Character (Backslash)](#221-escape-character-backslash), so escaped `<newline>` characters are already removed at this point.
2. [Redirection to or from filename] The expansions specified in [2.7 Redirection](#27-redirection) shall occur. As specified there, exactly one field can result (or the result is unspecified), and there are additional requirements on pathname expansion.
3. [Redirection from here-document] Quote removal shall be applied to the word to determine the delimiter that is used to find the end of the here-document that begins after the next `<newline>`.
4. [Case statement termination] When the **TOKEN** is exactly the reserved word **esac**, the token identifier for **esac** shall result. Otherwise, the token **WORD** shall be returned.
5. [**NAME** in **for**] When the **TOKEN** meets the requirements for a name (see XBD [*3.216 Name*](docs/posix/md/basedefs/V1_chap03.md#3216-name)), the token identifier **NAME** shall result. Otherwise, the token **WORD** shall be returned.
6. [Third word of **for** and **case**] (For a. and b.: As indicated in the grammar, a *linebreak* precedes the tokens **in** and **do**. If `<newline>` characters are present at the indicated location, it is the token after them that is treated in this fashion.)
    1. [**case** only] When the **TOKEN** is exactly the reserved word **in**, the token identifier for **in** shall result. Otherwise, the token **WORD** shall be returned.
    2. [**for** only] When the **TOKEN** is exactly the reserved word **in** or **do**, the token identifier for **in** or **do** shall result, respectively. Otherwise, the token **WORD** shall be returned.
7. [Assignment preceding command name] If a returned **ASSIGNMENT_WORD** token begins with a valid name, assignment of the value after the first `<equals-sign>` to the name shall occur as specified in [2.9.1 Simple Commands](#291-simple-commands). If a returned **ASSIGNMENT_WORD** token does not begin with a valid name, the way in which the token is processed is unspecified.
    1. [When the first word] If the **TOKEN** is exactly a reserved word, the token identifier for that reserved word shall result. Otherwise, 7b shall be applied.
    2. [Not the first word] If the **TOKEN** contains an unquoted (as determined while applying rule 4 from [2.3 Token Recognition](#23-token-recognition)) `<equals-sign>` character that is not part of an embedded parameter expansion, command substitution, or arithmetic expansion construct (as determined while applying rule 5 from [2.3 Token Recognition](#23-token-recognition)): Otherwise, the token **WORD** shall be returned.
          - If the **TOKEN** begins with `'='`, then the token **WORD** shall be returned.
          - If all the characters in the **TOKEN** preceding the first such `<equals-sign>` form a valid name (see XBD [*3.216 Name*](docs/posix/md/basedefs/V1_chap03.md#3216-name)), the token **ASSIGNMENT_WORD** shall be returned.
          - Otherwise, it is implementation-defined whether the token **WORD** or **ASSIGNMENT_WORD** is returned, or the **TOKEN** is processed in some other way.
8. [**NAME** in function] When the **TOKEN** is exactly a reserved word, the token identifier for that reserved word shall result. Otherwise, when the **TOKEN** meets the requirements for a name, the token identifier **NAME** shall result. Otherwise, rule 7 applies.
9. [Body of function] Word expansion and assignment shall never occur, even when required by the rules above, when this rule is being parsed. Each **TOKEN** that might either be expanded or have assignment applied to it shall instead be returned as a single **WORD** consisting only of characters that are exactly the token described in [2.3 Token Recognition](#23-token-recognition) .

```
/* -------------------------------------------------------
   The grammar symbols
   ------------------------------------------------------- */
%token  WORD
%token  ASSIGNMENT_WORD
%token  NAME
%token  NEWLINE
%token  IO_NUMBER
%token  IO_LOCATION
```

`/* The following are the operators (see XBD 3.243 Operator) containing more than one character. */`

```
%token  AND_IF    OR_IF    DSEMI    SEMI_AND
/*      '&&'      '||'     ';;'     ';&'   */

%token  DLESS  DGREAT  LESSAND  GREATAND  LESSGREAT  DLESSDASH
/*      '<<'   '>>'    '<&'     '>&'      '<>'       '<<-'   */

%token  CLOBBER
/*      '>|'   */

/* The following are the reserved words. */

%token  If    Then    Else    Elif    Fi    Do    Done
/*      'if'  'then'  'else'  'elif'  'fi'  'do'  'done'   */

%token  Case    Esac    While    Until    For
/*      'case'  'esac'  'while'  'until'  'for'   */

/* These are reserved words, not operator tokens, and are
   recognized when reserved words are recognized. */

%token  Lbrace    Rbrace    Bang
/*      '{'       '}'       '!'   */

%token  In
/*      'in'   */

/* -------------------------------------------------------
   The Grammar
   ------------------------------------------------------- */
%start program
%%
program          : linebreak complete_commands linebreak
                 | linebreak
                 ;
complete_commands: complete_commands newline_list complete_command
                 |                                complete_command
                 ;
complete_command : list separator_op
                 | list
                 ;
list             : list separator_op and_or
                 |                   and_or
                 ;
and_or           :                         pipeline
                 | and_or AND_IF linebreak pipeline
                 | and_or OR_IF  linebreak pipeline
                 ;
pipeline         :      pipe_sequence
                 | Bang pipe_sequence
                 ;
pipe_sequence    :                             command
                 | pipe_sequence '|' linebreak command
                 ;
command          : simple_command
                 | compound_command
                 | compound_command redirect_list
                 | function_definition
                 ;
compound_command : brace_group
                 | subshell
                 | for_clause
                 | case_clause
                 | if_clause
                 | while_clause
                 | until_clause
                 ;
subshell         : '(' compound_list ')'
                 ;
compound_list    : linebreak term
                 | linebreak term separator
                 ;
term             : term separator and_or
                 |                and_or
                 ;
for_clause       : For name                                      do_group
                 | For name                       sequential_sep do_group
                 | For name linebreak in          sequential_sep do_group
                 | For name linebreak in wordlist sequential_sep do_group
                 ;
name             : NAME                     /* Apply rule 5 */
                 ;
in               : In                       /* Apply rule 6 */
                 ;
wordlist         : wordlist WORD
                 |          WORD
                 ;
case_clause      : Case WORD linebreak in linebreak case_list    Esac
                 | Case WORD linebreak in linebreak case_list_ns Esac
                 | Case WORD linebreak in linebreak              Esac
                 ;
case_list_ns     : case_list case_item_ns
                 |           case_item_ns
                 ;
case_list        : case_list case_item
                 |           case_item
                 ;
case_item_ns     : pattern_list ')' linebreak
                 | pattern_list ')' compound_list
                 ;
case_item        : pattern_list ')' linebreak     DSEMI linebreak
                 | pattern_list ')' compound_list DSEMI linebreak
                 | pattern_list ')' linebreak     SEMI_AND linebreak
                 | pattern_list ')' compound_list SEMI_AND linebreak
                 ;
pattern_list     :                  WORD    /* Apply rule 4 */
                 |              '(' WORD    /* Do not apply rule 4 */
                 | pattern_list '|' WORD    /* Do not apply rule 4 */
                 ;
if_clause        : If compound_list Then compound_list else_part Fi
                 | If compound_list Then compound_list           Fi
                 ;
else_part        : Elif compound_list Then compound_list
                 | Elif compound_list Then compound_list else_part
                 | Else compound_list
                 ;
while_clause     : While compound_list do_group
                 ;
until_clause     : Until compound_list do_group
                 ;
function_definition : fname '(' ')' linebreak function_body
                 ;
function_body    : compound_command                /* Apply rule 9 */
                 | compound_command redirect_list  /* Apply rule 9 */
                 ;
fname            : NAME                            /* Apply rule 8 */
                 ;
brace_group      : Lbrace compound_list Rbrace
                 ;
do_group         : Do compound_list Done           /* Apply rule 6 */
                 ;
simple_command   : cmd_prefix cmd_word cmd_suffix
                 | cmd_prefix cmd_word
                 | cmd_prefix
                 | cmd_name cmd_suffix
                 | cmd_name
                 ;
cmd_name         : WORD                   /* Apply rule 7a */
                 ;
cmd_word         : WORD                   /* Apply rule 7b */
                 ;
cmd_prefix       :            io_redirect
                 | cmd_prefix io_redirect
                 |            ASSIGNMENT_WORD
                 | cmd_prefix ASSIGNMENT_WORD
                 ;
cmd_suffix       :            io_redirect
                 | cmd_suffix io_redirect
                 |            WORD
                 | cmd_suffix WORD
                 ;
redirect_list    :               io_redirect
                 | redirect_list io_redirect
                 ;
io_redirect      :             io_file
                 | IO_NUMBER   io_file
                 | IO_LOCATION io_file /* Optionally supported */
                 |             io_here
                 | IO_NUMBER   io_here
                 | IO_LOCATION io_here /* Optionally supported */
                 ;
io_file          : '<'       filename
                 | LESSAND   filename
                 | '>'       filename
                 | GREATAND  filename
                 | DGREAT    filename
                 | LESSGREAT filename
                 | CLOBBER   filename
                 ;
filename         : WORD                      /* Apply rule 2 */
                 ;
io_here          : DLESS     here_end
                 | DLESSDASH here_end
                 ;
here_end         : WORD                      /* Apply rule 3 */
                 ;
newline_list     :              NEWLINE
                 | newline_list NEWLINE
                 ;
linebreak        : newline_list
                 | /* empty */
                 ;
separator_op     : '&'
                 | ';'
                 ;
separator        : separator_op linebreak
                 | newline_list
                 ;
sequential_sep   : ';' linebreak
                 | newline_list
                 ;
```

### Tests

#### Test: function definition and invocation

Rule 1 recognizes reserved words like `{` and `}` in command position;
otherwise the token is returned as WORD. WORD expansion is deferred to
execution time.

```
begin test "function definition and invocation"
  script
    myfunc() {
        local_var="x"
        echo "$local_var"
    }
    myfunc
  expect
    stdout "x"
    stderr ""
    exit_code 0
end test "function definition and invocation"
```

#### Test: redirection filename expansion occurs at execution time

Rule 2 specifies that filename expansions for redirections occur at execution
time. A variable reference in a redirection target is expanded when the
command runs.

```
begin test "redirection filename expansion occurs at execution time"
  script
    FILE="/tmp/redir_grammar_test_$$"
    echo hello > "$FILE"
    cat < "$FILE"
    rm -f "$FILE"
  expect
    stdout "hello"
    stderr ""
    exit_code 0
end test "redirection filename expansion occurs at execution time"
```

#### Test: case statement recognizes in after newline

Rule 6a recognizes `in` in the third word position of a `case` statement even
when it follows a linebreak.

```
begin test "case statement recognizes in after newline"
  script
    case "foo"
    in
        foo) echo "matched" ;;
    esac
  expect
    stdout "matched"
    stderr ""
    exit_code 0
end test "case statement recognizes in after newline"
```

#### Test: quoted esac in case pattern is parsed as WORD

Rule 4 recognizes `esac` as the case terminator only when the token is exactly
the reserved word. A quoted `esac` token in a pattern list is parsed as WORD.

```
begin test "quoted esac in case pattern is parsed as WORD"
  script
    case esac in
      'esac') echo matched ;;
    esac
  expect
    stdout "matched"
    stderr ""
    exit_code 0
end test "quoted esac in case pattern is parsed as WORD"
```

#### Test: invalid identifier in for loop causes syntax error

Rule 5 requires the token after `for` to meet the requirements for a valid
NAME. An invalid identifier like `1invalid` fails this check and causes
a syntax error.

```
begin test "invalid identifier in for loop causes syntax error"
  script
    for 1invalid in a; do
      echo $1invalid
    done
  expect
    stdout ""
    stderr ".+"
    exit_code !=0
end test "invalid identifier in for loop causes syntax error"
```

#### Test: for loop recognizes do after newline

Rule 6b recognizes `do` in a `for` command even when it follows a linebreak.
This allows a `for name` loop with omitted `in` to parse correctly.

```
begin test "for loop recognizes do after newline"
  script
    set -- a b
    for i
    do
      echo "$i"
    done
  expect
    stdout "a\nb"
    stderr ""
    exit_code 0
end test "for loop recognizes do after newline"
```

#### Test: for loop name may be spelled in

Rule 5 applies in the `for` name position, so a token that meets the
requirements for a NAME is recognized as NAME there even if its text is `in`.

```
begin test "for loop name may be spelled in"
  script
    for in in a; do
      echo "$in"
    done
  expect
    stdout "a"
    stderr ""
    exit_code 0
end test "for loop name may be spelled in"
```

#### Test: token with equals after command name is WORD

Rule 7b applies only in positions where assignment recognition is relevant.
After the command name has been identified, a token containing `=` is treated
as a normal WORD argument.

```
begin test "token with equals after command name is WORD"
  script
    printf '%s\n' x=1
  expect
    stdout "x=1"
    stderr ""
    exit_code 0
end test "token with equals after command name is WORD"
```

#### Test: quoted reserved word is parsed as WORD

Rule 1 recognizes reserved words only when the token is exactly the reserved
word. A quoted `if` token is not the reserved word and is parsed as a simple
command name instead.

```
begin test "quoted reserved word is parsed as WORD"
  script
    'if' true 2>/dev/null
    echo $?
  expect
    stdout "127"
    stderr ""
    exit_code 0
end test "quoted reserved word is parsed as WORD"
```

#### Test: string starting with = is just a WORD

A token beginning with `=` is returned as WORD, not ASSIGNMENT_WORD
(rule 7b). Since `=foo` is not a valid command, it produces an error.

```
begin test "string starting with = is just a WORD"
  script
    =foo
  expect
    stdout ""
    stderr ".+"
    exit_code !=0
end test "string starting with = is just a WORD"
```

#### Test: function name token identification

Rule 8 identifies the token after a function name position. When the token
meets the requirements for a NAME, it is recognized as the function name.

```
begin test "function name token identification"
  script
    myfn() { echo "fn-ok"; }
    myfn
  expect
    stdout "fn-ok"
    stderr ""
    exit_code 0
end test "function name token identification"
```

#### Test: reserved word cannot be function name

Rule 8 first checks whether the token is exactly a reserved word. An exact
reserved word such as `if` is not recognized as NAME in function-name
position, so the definition is a syntax error.

```
begin test "reserved word cannot be function name"
  script
    if() { echo bad; }
  expect
    stdout ""
    stderr "(.|\n)+"
    exit_code !=0
end test "reserved word cannot be function name"
```

#### Test: function body expansion deferred to invocation

Rule 9 specifies that word expansion in the function body is deferred —
it never occurs at definition time, only when the function is invoked.

```
begin test "function body expansion deferred to invocation"
  script
    X=before
    fn() { echo "$X"; }
    X=after
    fn
  expect
    stdout "after"
    stderr ""
    exit_code 0
end test "function body expansion deferred to invocation"
```

#### Test: multi-digit file descriptor as IO_NUMBER

Rule 2 says a string "consisting solely of digits" before `<` or `>`
is classified as IO_NUMBER. This applies to multi-digit descriptors like
10, not just single digits.

```
begin test "multi-digit file descriptor as IO_NUMBER"
  script
    exec 10>tmp_fd10.txt
    echo hello >&10
    exec 10>&-
    cat tmp_fd10.txt
  expect
    stdout "hello"
    stderr ""
    exit_code 0
end test "multi-digit file descriptor as IO_NUMBER"
```

#### Test: reserved word in non-reserved position is WORD

Rule 1 (2.10.2) states that reserved words are only recognized as such in
positions where a reserved word could be the next correct token. In
argument position, `if`, `then`, `else`, and `fi` are ordinary WORDs.

```
begin test "reserved word in non-reserved position is WORD"
  script
    echo if then else fi
  expect
    stdout "if then else fi"
    stderr ""
    exit_code 0
end test "reserved word in non-reserved position is WORD"
```

#### Test: bang negates pipeline exit status

The grammar production `pipeline : Bang pipe_sequence` defines `!` as
negating the exit status of a pipeline. A successful command negated
yields non-zero and vice versa.

```
begin test "bang negates pipeline exit status"
  script
    ! false
    echo $?
    ! true
    echo $?
  expect
    stdout "0\n1"
    stderr ""
    exit_code 0
end test "bang negates pipeline exit status"
```

#### Test: newline allowed after AND_IF operator

The grammar production `and_or : and_or AND_IF linebreak pipeline`
includes a linebreak non-terminal after `&&`, allowing newlines between
the operator and the next pipeline.

```
begin test "newline allowed after AND_IF operator"
  script
    true &&
    echo ok
  expect
    stdout "ok"
    stderr ""
    exit_code 0
end test "newline allowed after AND_IF operator"
```

#### Test: newline allowed after OR_IF operator

The grammar production `and_or : and_or OR_IF linebreak pipeline`
includes a linebreak after `||`, allowing newlines between the operator
and the next pipeline.

```
begin test "newline allowed after OR_IF operator"
  script
    false ||
    echo ok
  expect
    stdout "ok"
    stderr ""
    exit_code 0
end test "newline allowed after OR_IF operator"
```

#### Test: newline allowed after pipe operator

The grammar production `pipe_sequence : pipe_sequence '|' linebreak
command` includes a linebreak after `|`, allowing newlines between the
pipe operator and the next command.

```
begin test "newline allowed after pipe operator"
  script
    echo hello |
    cat
  expect
    stdout "hello"
    stderr ""
    exit_code 0
end test "newline allowed after pipe operator"
```

#### Test: subshell as standalone compound command

The grammar production `compound_command : subshell` allows `( ... )`
as a compound command. Commands inside the subshell execute in a child
environment, so variable changes do not affect the parent.

```
begin test "subshell as standalone compound command"
  script
    x=outer
    (x=inner; echo "$x")
    echo "$x"
  expect
    stdout "inner\nouter"
    stderr ""
    exit_code 0
end test "subshell as standalone compound command"
```

#### Test: compound command with redirect list

The grammar production `command : compound_command redirect_list` allows
redirections to be applied to an entire compound command such as a brace
group.

```
begin test "compound command with redirect list"
  script
    { echo hello; echo world; } > tmp_brace_redir.txt
    cat tmp_brace_redir.txt
  expect
    stdout "hello\nworld"
    stderr ""
    exit_code 0
end test "compound command with redirect list"
```

#### Test: case pattern with leading parenthesis

The grammar production `pattern_list : '(' WORD` allows case patterns
to begin with an optional `(`, which is sometimes used for symmetry.

```
begin test "case pattern with leading parenthesis"
  script
    case hello in
      (hello) echo matched ;;
    esac
  expect
    stdout "matched"
    stderr ""
    exit_code 0
end test "case pattern with leading parenthesis"
```

#### Test: case pattern with pipe alternatives

The grammar production `pattern_list : pattern_list '|' WORD` allows
multiple alternative patterns separated by `|` in a single case item.

```
begin test "case pattern with pipe alternatives"
  script
    case b in
      a|b|c) echo matched ;;
    esac
  expect
    stdout "matched"
    stderr ""
    exit_code 0
end test "case pattern with pipe alternatives"
```

#### Test: last case item may omit terminator

The grammar's case_item_ns production allows the final item in a case
statement to omit the `;;` or `;&` terminator.

```
begin test "last case item may omit terminator"
  script
    case x in
      x) echo matched
    esac
  expect
    stdout "matched"
    stderr ""
    exit_code 0
end test "last case item may omit terminator"
```

#### Test: empty case clause matches nothing

The grammar production `case_clause : Case WORD linebreak in linebreak
Esac` allows a case statement with no patterns at all. The case command
completes with exit status zero.

```
begin test "empty case clause matches nothing"
  script
    case x in
    esac
    echo $?
  expect
    stdout "0"
    stderr ""
    exit_code 0
end test "empty case clause matches nothing"
```

#### Test: for loop with in but empty word list iterates zero times

The grammar production `For name linebreak in sequential_sep do_group`
allows `for x in ;` with an empty wordlist, meaning the loop body
executes zero times.

```
begin test "for loop with in but empty word list iterates zero times"
  script
    for x in; do
      echo bad
    done
    echo ok
  expect
    stdout "ok"
    stderr ""
    exit_code 0
end test "for loop with in but empty word list iterates zero times"
```

#### Test: for loop recognizes in after newline

The grammar production `For name linebreak in wordlist sequential_sep
do_group` includes a linebreak before `in`, allowing newlines between
the loop variable name and the `in` keyword. Rule 6b recognizes `in`
in this position.

```
begin test "for loop recognizes in after newline"
  script
    for x
    in a b; do
      echo "$x"
    done
  expect
    stdout "a\nb"
    stderr ""
    exit_code 0
end test "for loop recognizes in after newline"
```

#### Test: background command with ampersand separator

The grammar production `separator_op : '&'` causes the preceding
command to execute asynchronously. The shell continues to the next
command without waiting.

```
begin test "background command with ampersand separator"
  script
    echo bg > tmp_bg_grammar.txt &
    wait
    cat tmp_bg_grammar.txt
    echo fg
  expect
    stdout "bg\nfg"
    stderr ""
    exit_code 0
end test "background command with ampersand separator"
```

#### Test: io_redirect in cmd_prefix before command name

The grammar production `cmd_prefix : io_redirect` allows redirections
to appear before the command name in a simple command.

```
begin test "io_redirect in cmd_prefix before command name"
  script
    >tmp_prefix_grammar.txt echo hello
    cat tmp_prefix_grammar.txt
  expect
    stdout "hello"
    stderr ""
    exit_code 0
end test "io_redirect in cmd_prefix before command name"
```

#### Test: multiple assignment words in cmd_prefix

The recursive grammar production `cmd_prefix : cmd_prefix
ASSIGNMENT_WORD` allows multiple consecutive variable assignments before
the command name.

```
begin test "multiple assignment words in cmd_prefix"
  script
    A=1 B=2 C=3 sh -c 'echo $A $B $C'
  expect
    stdout "1 2 3"
    stderr ""
    exit_code 0
end test "multiple assignment words in cmd_prefix"
```

#### Test: while clause as compound command

The grammar production `while_clause : While compound_list do_group`
defines a while loop. The condition compound_list is evaluated before
each iteration; the loop repeats while it exits zero.

```
begin test "while clause as compound command"
  script
    n=0
    while [ "$n" -lt 3 ]; do
      n=$((n + 1))
    done
    echo "$n"
  expect
    stdout "3"
    stderr ""
    exit_code 0
end test "while clause as compound command"
```

#### Test: until clause as compound command

The grammar production `until_clause : Until compound_list do_group`
defines an until loop. The loop body repeats until the condition
compound_list exits zero.

```
begin test "until clause as compound command"
  script
    n=0
    until [ "$n" -ge 3 ]; do
      n=$((n + 1))
    done
    echo "$n"
  expect
    stdout "3"
    stderr ""
    exit_code 0
end test "until clause as compound command"
```

#### Test: simple if-then-fi without else part

The grammar production `if_clause : If compound_list Then compound_list
Fi` defines an if statement with no else_part. When the condition is true
the body executes; when false, the if command exits zero silently.

```
begin test "simple if-then-fi without else part"
  script
    if true; then
      echo yes
    fi
    if false; then
      echo no
    fi
    echo done
  expect
    stdout "yes\ndone"
    stderr ""
    exit_code 0
end test "simple if-then-fi without else part"
```

#### Test: if clause with else part

The grammar productions `if_clause : If compound_list Then compound_list
else_part Fi` and `else_part : Else compound_list` define the if-then-else
construct. The else branch executes when the condition is non-zero.

```
begin test "if clause with else part"
  script
    if false; then
      echo bad
    else
      echo good
    fi
  expect
    stdout "good"
    stderr ""
    exit_code 0
end test "if clause with else part"
```

#### Test: elif chain in if clause

The grammar production `else_part : Elif compound_list Then compound_list
else_part` allows chained elif branches. Each condition is tried in order
until one succeeds.

```
begin test "elif chain in if clause"
  script
    x=2
    if [ "$x" = 1 ]; then
      echo one
    elif [ "$x" = 2 ]; then
      echo two
    elif [ "$x" = 3 ]; then
      echo three
    else
      echo other
    fi
  expect
    stdout "two"
    stderr ""
    exit_code 0
end test "elif chain in if clause"
```

#### Test: elif without trailing else clause

The grammar production `else_part : Elif compound_list Then
compound_list` allows an elif branch with no final else. When
neither the if nor the elif condition is true, no branch executes.

```
begin test "elif without trailing else clause"
  script
    x=3
    if [ "$x" = 1 ]; then
      echo one
    elif [ "$x" = 2 ]; then
      echo two
    fi
    echo done
  expect
    stdout "done"
    stderr ""
    exit_code 0
end test "elif without trailing else clause"
```

#### Test: function body with redirect list in definition

The grammar production `function_body : compound_command redirect_list`
allows redirections to be part of the function definition. They are
applied each time the function is called.

```
begin test "function body with redirect list in definition"
  script
    fn() { echo "from fn"; } > tmp_fn_redir.txt
    fn
    cat tmp_fn_redir.txt
  expect
    stdout "from fn"
    stderr ""
    exit_code 0
end test "function body with redirect list in definition"
```

#### Test: DLESSDASH strips leading tabs from here-document

The grammar production `io_here : DLESSDASH here_end` recognizes the
`<<-` operator, which strips leading tab characters from each line of
the here-document body and from the delimiter line.

```
begin test "DLESSDASH strips leading tabs from here-document"
  script
    eval "$(printf 'cat <<-ENDHERE\n\thello world\n\tENDHERE\n')"
  expect
    stdout "hello world"
    stderr ""
    exit_code 0
end test "DLESSDASH strips leading tabs from here-document"
```

#### Test: IO_NUMBER with here-document

The grammar production `io_redirect : IO_NUMBER io_here` allows a
here-document to be directed to an explicit file descriptor rather
than the default standard input.

```
begin test "IO_NUMBER with here-document"
  script
    exec 3<<EOF
    hello from fd3
    EOF
    cat <&3
  expect
    stdout "hello from fd3"
    stderr ""
    exit_code 0
end test "IO_NUMBER with here-document"
```

#### Test: LESSAND duplicates input file descriptor

The grammar production `io_file : LESSAND filename` recognizes the `<&`
multi-character operator, which duplicates an input file descriptor.

```
begin test "LESSAND duplicates input file descriptor"
  script
    echo hello > tmp_lessand.txt
    exec 3<tmp_lessand.txt
    cat <&3
    exec 3<&-
  expect
    stdout "hello"
    stderr ""
    exit_code 0
end test "LESSAND duplicates input file descriptor"
```

#### Test: append redirect with DGREAT operator

The grammar production `io_file : DGREAT filename` recognizes the `>>`
multi-character operator token, which appends output to an existing file
instead of truncating it.

```
begin test "append redirect with DGREAT operator"
  script
    echo first > tmp_dgreat.txt
    echo second >> tmp_dgreat.txt
    cat tmp_dgreat.txt
  expect
    stdout "first\nsecond"
    stderr ""
    exit_code 0
end test "append redirect with DGREAT operator"
```

#### Test: LESSGREAT opens file for reading and writing

The grammar production `io_file : LESSGREAT filename` recognizes the
`<>` multi-character operator, which opens a file for both reading and
writing.

```
begin test "LESSGREAT opens file for reading and writing"
  script
    echo hello > tmp_lessgreat.txt
    cat <> tmp_lessgreat.txt
  expect
    stdout "hello"
    stderr ""
    exit_code 0
end test "LESSGREAT opens file for reading and writing"
```

#### Test: clobber operator overrides noclobber

The grammar defines `CLOBBER` (`>|`) as an io_file operator. It forces
output redirection to overwrite an existing file even when the shell's
noclobber option (`set -C`) is active.

```
begin test "clobber operator overrides noclobber"
  script
    set -C
    echo first > tmp_clobber.txt
    echo second >| tmp_clobber.txt
    cat tmp_clobber.txt
    set +C
  expect
    stdout "second"
    stderr ""
    exit_code 0
end test "clobber operator overrides noclobber"
```

#### Test: SEMI_AND case fallthrough

The grammar production `case_item : pattern_list ')' compound_list
SEMI_AND linebreak` uses `;&` to fall through to the next case item's
body without re-evaluating the pattern.

```
begin test "SEMI_AND case fallthrough"
  script
    case x in
      x) echo one ;&
      y) echo two ;;
    esac
  expect
    stdout "one\ntwo"
    stderr ""
    exit_code 0
end test "SEMI_AND case fallthrough"
```

#### Test: empty case body with DSEMI terminator

The grammar production `case_item : pattern_list ')' linebreak DSEMI
linebreak` allows a non-last case item to have an empty body while
still using `;;` to separate it from subsequent items.

```
begin test "empty case body with DSEMI terminator"
  script
    case x in
      y) ;;
      x) echo matched ;;
    esac
  expect
    stdout "matched"
    stderr ""
    exit_code 0
end test "empty case body with DSEMI terminator"
```

#### Test: last case item with empty body exits zero

The grammar production `case_item_ns : pattern_list ')' linebreak`
allows the final case item to have an empty body (no compound_list).
When matched, the case command exits zero.

```
begin test "last case item with empty body exits zero"
  script
    case x in
      x)
    esac
    echo $?
  expect
    stdout "0"
    stderr ""
    exit_code 0
end test "last case item with empty body exits zero"
```

#### Test: simple command with only redirect in cmd_prefix

The grammar production `simple_command : cmd_prefix` allows a command
consisting of only redirections with no command name. A bare `>file`
creates or truncates the file.

```
begin test "simple command with only redirect in cmd_prefix"
  script
    echo content > tmp_prefix_only.txt
    >tmp_prefix_only.txt
    [ -s tmp_prefix_only.txt ] && echo notempty || echo empty
  expect
    stdout "empty"
    stderr ""
    exit_code 0
end test "simple command with only redirect in cmd_prefix"
```

#### Test: multiple redirects in redirect_list on compound command

The recursive grammar production `redirect_list : redirect_list
io_redirect` allows multiple redirections to be applied to a compound
command. Both stdout and stderr can be redirected independently.

```
begin test "multiple redirects in redirect_list on compound command"
  script
    { echo out; echo err >&2; } > tmp_multi_out.txt 2> tmp_multi_err.txt
    cat tmp_multi_out.txt
    cat tmp_multi_err.txt
  expect
    stdout "out\nerr"
    stderr ""
    exit_code 0
end test "multiple redirects in redirect_list on compound command"
```

#### Test: multiple redirects in cmd_suffix

The recursive grammar production `cmd_suffix : cmd_suffix io_redirect`
allows multiple redirections after the command name. Both stdout and
stderr can be separately redirected in a single simple command.

```
begin test "multiple redirects in cmd_suffix"
  script
    echo hello >tmp_suf_out.txt 2>tmp_suf_err.txt
    cat tmp_suf_out.txt
  expect
    stdout "hello"
    stderr ""
    exit_code 0
end test "multiple redirects in cmd_suffix"
```

#### Test: reserved word text before equals is ASSIGNMENT_WORD

Rule 7a checks whether the TOKEN is exactly a reserved word. The token
`if=hello` is not exactly `if`, so rule 7b applies. Since the characters
before the first `=` form a valid name, the token is classified as
ASSIGNMENT_WORD — the variable `if` is assigned.

```
begin test "reserved word text before equals is ASSIGNMENT_WORD"
  script
    if=hello
    echo "$if"
  expect
    stdout "hello"
    stderr ""
    exit_code 0
end test "reserved word text before equals is ASSIGNMENT_WORD"
```

#### Test: unquoted here-document delimiter allows parameter expansion

Rule 3 applies quote removal to determine the here-document delimiter.
When the delimiter word contains no quoting, expansions inside the
here-document body are performed normally.

```
begin test "unquoted here-document delimiter allows parameter expansion"
  script
    var=hello
    cat <<EOF
    $var
    EOF
  expect
    stdout "hello"
    stderr ""
    exit_code 0
end test "unquoted here-document delimiter allows parameter expansion"
```

#### Test: for loop with do on same line and no separator

The first grammar production `for_clause : For name do_group` has no
separator between the loop variable name and the `do` keyword. This
allows `for i do ... done` on a single line without a semicolon
or newline between the name and `do`.

```
begin test "for loop with do on same line and no separator"
  script
    set -- a b c
    for i do echo "$i"; done
  expect
    stdout "a\nb\nc"
    stderr ""
    exit_code 0
end test "for loop with do on same line and no separator"
```

#### Test: empty body with SEMI_AND fallthrough

The grammar production `case_item : pattern_list ')' linebreak
SEMI_AND linebreak` allows a case item with an empty body to fall
through via `;&` to the next item's body.

```
begin test "empty body with SEMI_AND fallthrough"
  script
    case x in
      x) ;&
      y) echo matched ;;
    esac
  expect
    stdout "matched"
    stderr ""
    exit_code 0
end test "empty body with SEMI_AND fallthrough"
```

#### Test: GREATAND duplicates output file descriptor

The grammar production `io_file : GREATAND filename` recognizes the
`>&` multi-character operator, which duplicates an output file
descriptor to the specified target.

```
begin test "GREATAND duplicates output file descriptor"
  script
    echo hello >&2
  expect
    stdout ""
    stderr "hello"
    exit_code 0
end test "GREATAND duplicates output file descriptor"
```

#### Test: multiple redirects in cmd_prefix

The recursive grammar production `cmd_prefix : cmd_prefix io_redirect`
allows multiple redirections to appear before the command name. Both
stdout and stderr can be redirected in prefix position.

```
begin test "multiple redirects in cmd_prefix"
  script
    >tmp_pre_out.txt 2>tmp_pre_err.txt echo hello
    cat tmp_pre_out.txt
  expect
    stdout "hello"
    stderr ""
    exit_code 0
end test "multiple redirects in cmd_prefix"
```

#### Test: terminated items before unterminated last item

The grammar production `case_list_ns : case_list case_item_ns` allows
a case statement with `;;`-terminated items followed by a final item
that omits the terminator.

```
begin test "terminated items before unterminated last item"
  script
    case x in
      a) echo a ;;
      x) echo x
    esac
  expect
    stdout "x"
    stderr ""
    exit_code 0
end test "terminated items before unterminated last item"
```

#### Test: line joining produces reserved word before tokenization

The Rule 1 Note states that line joining (backslash-newline removal)
is done before tokenization. A reserved word split across lines with
an escaped newline is reassembled and recognized as the reserved word.

```
begin test "line joining produces reserved word before tokenization"
  script
    i\
    f true; then echo yes; fi
  expect
    stdout "yes"
    stderr ""
    exit_code 0
end test "line joining produces reserved word before tokenization"
```

#### Test: dollar-sign single-quote prevents reserved word recognition

The Rule 1 Note lists `$'...'` as a quoting character retained in the
token during reserved word classification. Since `$'if'` is not exactly
the token `if`, it is returned as WORD, not as the `If` reserved word.

```
begin test "dollar-sign single-quote prevents reserved word recognition"
  script
    $'if' true 2>/dev/null
    echo $?
  expect
    stdout "127"
    stderr ""
    exit_code 0
end test "dollar-sign single-quote prevents reserved word recognition"
```

#### Test: for wordlist with newline as sequential separator

The grammar production `For name linebreak in wordlist sequential_sep
do_group` allows a newline (via `sequential_sep : newline_list`) between
the last word in the wordlist and the `do` keyword.

```
begin test "for wordlist with newline as sequential separator"
  script
    for x in a b c
    do
      echo "$x"
    done
  expect
    stdout "a\nb\nc"
    stderr ""
    exit_code 0
end test "for wordlist with newline as sequential separator"
```
#### Test: empty program is valid grammar
```
begin test "empty program is valid grammar"
  script
    
    
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "empty program is valid grammar"
```

#### Test: equals sign in embedded expansion construct yields WORD
```
begin test "equals sign in embedded expansion construct yields WORD"
  script
    unset a
    $(echo a=b) 2>/dev/null
    echo "a=${a-unset}"
  expect
    stdout "a=unset"
    stderr ""
    exit_code 0
end test "equals sign in embedded expansion construct yields WORD"
```

#### Test: quoted in in case statement is parsed as WORD
```
begin test "quoted in in case statement is parsed as WORD"
  script
    case foo 'in' foo) echo ok ;; esac
  expect
    stdout ""
    stderr "(.|\n)+"
    exit_code !=0
end test "quoted in in case statement is parsed as WORD"
```

#### Test: quoted in in for loop is parsed as WORD
```
begin test "quoted in in for loop is parsed as WORD"
  script
    for x "in" a b; do echo "$x"; done
  expect
    stdout ""
    stderr "(.|\n)+"
    exit_code !=0
end test "quoted in in for loop is parsed as WORD"
```

#### Test: quoted do in for loop is parsed as WORD
```
begin test "quoted do in for loop is parsed as WORD"
  script
    for x in a b; "do" echo "$x"; done
  expect
    stdout ""
    stderr "(.|\n)+"
    exit_code !=0
end test "quoted do in for loop is parsed as WORD"
```

#### Test: for loop with name and semicolon separator
```
begin test "for loop with name and semicolon separator"
  script
    set -- a b
    for i; do
      echo "$i"
    done
  expect
    stdout "a\nb"
    stderr ""
    exit_code 0
end test "for loop with name and semicolon separator"
```
#### Test: empty subshell is a syntax error
```
begin test "empty subshell is a syntax error"
  script
    ( )
  expect
    stdout ""
    stderr "(.|\n)+"
    exit_code !=0
end test "empty subshell is a syntax error"
```

#### Test: empty brace group is a syntax error
```
begin test "empty brace group is a syntax error"
  script
    { }
  expect
    stdout ""
    stderr "(.|\n)+"
    exit_code !=0
end test "empty brace group is a syntax error"
```

#### Test: empty do group is a syntax error
```
begin test "empty do group is a syntax error"
  script
    for i in a; do done
  expect
    stdout ""
    stderr "(.|\n)+"
    exit_code !=0
end test "empty do group is a syntax error"
```

#### Test: empty then clause is a syntax error
```
begin test "empty then clause is a syntax error"
  script
    if true; then fi
  expect
    stdout ""
    stderr "(.|\n)+"
    exit_code !=0
end test "empty then clause is a syntax error"
```

#### Test: empty if condition is a syntax error
```
begin test "empty if condition is a syntax error"
  script
    if then echo a; fi
  expect
    stdout ""
    stderr "(.|\n)+"
    exit_code !=0
end test "empty if condition is a syntax error"
```

#### Test: multiple bangs in pipeline cause syntax error

The grammar permits exactly one `Bang` (`!`) per pipeline. Repeating it
is a syntax error. (Note: `bash --posix` incorrectly accepts this as an
extension.)

```
begin test "multiple bangs in pipeline cause syntax error"
  script
    ! ! true
  expect
    stdout ""
    stderr ".+"
    exit_code 2
end test "multiple bangs in pipeline cause syntax error"
```

#### Test: bang negates compound command
```
begin test "bang negates compound command"
  script
    ! { false; }
    echo $?
  expect
    stdout "0"
    stderr ""
    exit_code 0
end test "bang negates compound command"
```

#### Test: bang negates entire pipe sequence
```
begin test "bang negates entire pipe sequence"
  script
    ! false | true
    echo $?
    ! true | false
    echo $?
  expect
    stdout "1\n0"
    stderr ""
    exit_code 0
end test "bang negates entire pipe sequence"
```
#### Test: empty sequential list does not execute

The grammar does not allow an empty command before a semicolon separator.
A stray semicolon causes a syntax error.

```
begin test "empty sequential list does not execute"
  script
    ;
  expect
    stdout ""
    stderr "(.|\n)+"
    exit_code !=0
end test "empty sequential list does not execute"
```

#### Test: double semicolon not in case statement

The `DSEMI` token (`;;`) is only grammatically valid within a `case`
command to terminate a case item. Using it elsewhere is a syntax error.

```
begin test "double semicolon not in case statement"
  script
    echo a ;; echo b
  expect
    stdout ""
    stderr "(.|\n)+"
    exit_code !=0
end test "double semicolon not in case statement"
```

#### Test: IO_LOCATION token is parsed as redirection

Rule 3 allows a string like `{fd}` immediately preceding `<` or `>`
to be classified as `IO_LOCATION`. This is an optional feature. If supported,
it assigns the file descriptor to the named variable; if not supported,
it should be a syntax error or a literal string, but the shell should
handle it gracefully.

```
begin test "IO_LOCATION token is parsed as redirection"
  script
    {fd}>tmp_ioloc.txt echo ok 2>/dev/null || echo syntax_error
    rm -f tmp_ioloc.txt
  expect
    stdout "(ok|syntax_error)"
    stderr ""
    exit_code 0
end test "IO_LOCATION token is parsed as redirection"
```
#### Test: empty wordlist in for loop executes zero times

The grammar allows an empty wordlist via `For name linebreak in sequential_sep do_group`.
If the wordlist is omitted, the loop body executes zero times.

```
begin test "empty wordlist in for loop executes zero times"
  script
    for i in; do echo "iter"; done
    echo "done"
  expect
    stdout "done"
    stderr ""
    exit_code 0
end test "empty wordlist in for loop executes zero times"
```

#### Test: compound list execution order

A `compound_list` executes its terms in sequence.

```
begin test "compound list execution order"
  script
    {
      echo 1
      echo 2
      echo 3
    }
  expect
    stdout "1\n2\n3"
    stderr ""
    exit_code 0
end test "compound list execution order"
```

#### Test: background brace group

A brace group can be executed asynchronously by suffixing it with `&`.

```
begin test "background brace group"
  script
    { sleep 0.1; echo bg; } > tmp_bg_brace.txt &
    wait
    cat tmp_bg_brace.txt
  expect
    stdout "bg"
    stderr ""
    exit_code 0
end test "background brace group"
```

#### Test: background subshell

A subshell can be executed asynchronously by suffixing it with `&`.

```
begin test "background subshell"
  script
    ( sleep 0.1; echo bg ) > tmp_bg_subshell.txt &
    wait
    cat tmp_bg_subshell.txt
  expect
    stdout "bg"
    stderr ""
    exit_code 0
end test "background subshell"
```
