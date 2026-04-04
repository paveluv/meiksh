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

#### Test: quoted here-document delimiter suppresses expansion

When the here-document delimiter is quoted, the token identification rules
preserve the quoting, and no parameter expansion is performed within the
here-document body.

```
begin test "quoted here-document delimiter suppresses expansion"
  script
    cat << "EOF"
    $var
    EOF
  expect
    stdout "\$var"
    stderr ""
    exit_code 0
end test "quoted here-document delimiter suppresses expansion"
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

#### Test: case statement with in and esac

Rule 4 (case termination) recognizes `esac` as a reserved word; rule 6a
recognizes `in` in the third word position of a case statement.

```
begin test "case statement with in and esac"
  script
    case "foo" in
        foo) echo "matched" ;;
    esac
  expect
    stdout "matched"
    stderr ""
    exit_code 0
end test "case statement with in and esac"
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

#### Test: for-in loop with valid identifier

When the token after `for` is a valid NAME (rule 5) and `in` is recognized
as a reserved word (rule 6b), the for loop parses and executes correctly.

```
begin test "for-in loop with valid identifier"
  script
    for i in a; do
      echo $i
    done
  expect
    stdout "a"
    stderr ""
    exit_code 0
end test "for-in loop with valid identifier"
```

#### Test: valid assignment prefix scopes to command

Rule 7 identifies ASSIGNMENT_WORD tokens in the command prefix. The
assignment `var=1` is scoped to the command's environment.

```
begin test "valid assignment prefix scopes to command"
  script
    var=1 env | grep -q "^var=1$" && echo "assigned"
  expect
    stdout "assigned"
    stderr ""
    exit_code 0
end test "valid assignment prefix scopes to command"
```

#### Test: invalid name cannot be assignment

A token that begins with a digit cannot form a valid name, so it is not
recognized as ASSIGNMENT_WORD and causes an error.

```
begin test "invalid name cannot be assignment"
  script
    1invalid=true sh -c "echo executed"
  expect
    stdout ""
    stderr ".+"
    exit_code !=0
end test "invalid name cannot be assignment"
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
