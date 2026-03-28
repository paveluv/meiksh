# POSIX Shell Test Suite

This document translates normative POSIX requirements into an actionable test plan for `meiksh`.
The generated `SHALL-XX-YY-ZZZ` identifiers map directly to test scripts (e.g., `tests/spec/SHALL-19-09-001.sh`).

---

## Shell Command Language

### 2. Shell Command Language > Quoting

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-02-001` | Verify that: The application **must** quote the following characters if they are to represent themselves: |

### 2. Shell Command Language > Quoting > Escape Character (Backslash)

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-02-01-001` | Verify that: A <backslash> that is not quoted **must** preserve the literal value of the following character, with the exception of a <newline>. If a <newline> immediately follows the <backslash>, the shell **must** interpret this as line continuation. The <backslash> and <newline> **must** be removed before splitting the input into tokens. Since the escaped <newline> is removed entirely from the input and is not replaced by any white space, it cannot serve as a token separator. |

### 2. Shell Command Language > Quoting > Single-Quotes

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-02-02-001` | Verify that: Enclosing characters in single-quotes ('') **must** preserve the literal value of each character within the single-quotes. A single-quote cannot occur within single-quotes. |

### 2. Shell Command Language > Quoting > Double-Quotes

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-02-03-001` | Verify that: Enclosing characters in double-quotes ("") **must** preserve the literal value of all characters within the double-quotes, with the exception of the characters backquote, <dollar-sign>, and <backslash>, as follows: |
| `SHALL-19-02-03-002` | Verify that: The input characters within the quoted string that are also enclosed between "$(" and the matching ')' **must** not be affected by the double-quotes, but rather **must** define the command(s) whose output replaces the "$(...)" when the word is expanded. The tokenizing rules in 2.3 Token Recognition **must** be applied recursively to find the matching ')'. |
| `SHALL-19-02-03-003` | Verify that: For the four varieties of parameter expansion that provide for substring processing (see 2.6.2 Parameter Expansion), within the string of characters from an enclosed "${" to the matching '}', the double-quotes within which the expansion occurs **must** have no effect on the handling of any special characters. |
| `SHALL-19-02-03-004` | Verify that: When double-quotes are used to quote a parameter expansion, command substitution, or arithmetic expansion, the literal value of all characters within the result of the expansion **must** be preserved. |
| `SHALL-19-02-03-005` | Verify that: The application **must** ensure that a double-quote that is not within "$(...)" nor within "${...}" is immediately preceded by a <backslash> in order to be included within double-quotes. The parameter '@' has special meaning inside double-quotes and is described in 2.5.2 Special Parameters. |

### 2. Shell Command Language > Quoting > Dollar-Single-Quotes

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-02-04-001` | Verify that: A sequence of characters starting with a <dollar-sign> immediately followed by a single-quote ($') **must** preserve the literal value of all characters up to an unescaped terminating single-quote ('), with the exception of certain <backslash>-escape sequences, as follows: |
| `SHALL-19-02-04-002` | Verify that: In cases where a variable number of characters can be used to specify an escape sequence (\xXX and \ddd), the escape sequence **must** be terminated by the first character that is not of the expected type or, for \ddd sequences, when the maximum number of characters specified has been found, whichever occurs first. |
| `SHALL-19-02-04-003` | Verify that: These <backslash>-escape sequences **must** be processed (replaced with the bytes or characters they yield) immediately prior to word expansion (see 2.6 Word Expansions) of the word in which the dollar-single-quotes sequence occurs. |
| `SHALL-19-02-04-004` | Verify that: If a \e or \cX escape sequence specifies a character that does not have an encoding in the locale in effect when these <backslash>-escape sequences are processed, the result is implementation-defined. However, implementations **must** not replace an unsupported character with bytes that do not form valid characters in that locale's character set. |
| `SHALL-19-02-04-005` | Verify that: If a <backslash>-escape sequence represents a single-quote character (for example \'), that sequence **must** not terminate the dollar-single-quote sequence. |

### 2. Shell Command Language > Token Recognition

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-03-001` | Verify that: The shell **must** read its input in terms of lines. (For details about how the shell reads its input, see the description of sh.) The input lines can be of unlimited length. These lines **must** be parsed using two major modes: ordinary token recognition and processing of here-documents. |
| `SHALL-19-03-002` | Verify that: When an io_here token has been recognized by the grammar (see 2.10 Shell Grammar), one or more of the subsequent lines immediately following the next NEWLINE token form the body of a here-document and **must** be parsed according to the rules of 2.7.4 Here-Document. Any non-NEWLINE tokens (including more io_here tokens) that are recognized while searching for the next NEWLINE token **must** be saved for processing after the here-document has been parsed. If a saved token is an io_here token, the corresponding here-document **must** start on the line immediately following the line containing the trailing delimiter of the previous here-document. If any saved token includes a <newline> character, the behavior is unspecified. |
| `SHALL-19-03-003` | Verify that: When it is not processing an io_here, the shell **must** break its input into tokens by applying the first applicable rule below to each character in turn in its input. At the start of input or after a previous token has just been delimited, the first or next token, respectively, **must** start with the first character that has not already been included in a token and is not discarded according to the rules below. Once a token has started, zero or more characters from the input **must** be appended to the token until the end of the token is delimited according to one of the rules below. When both the start and end of a token have been delimited, the characters forming the token **must** be exactly those in the input between the two delimiters, including any quoting characters. If a rule below indicates that a token is delimited, and no characters have been included in the token, that empty token **must** be discarded. |
| `SHALL-19-03-004` | Verify that: If the end of input is recognized, the current token (if any) **must** be delimited. |
| `SHALL-19-03-005` | Verify that: If the previous character was used as part of an operator and the current character is not quoted and can be used with the previous characters to form an operator, it **must** be used as part of that (operator) token. |
| `SHALL-19-03-006` | Verify that: If the previous character was used as part of an operator and the current character cannot be used with the previous characters to form an operator, the operator containing the previous character **must** be delimited. |
| `SHALL-19-03-007` | Verify that: If the current character is an unquoted <backslash>, single-quote, or double-quote or is the first character of an unquoted <dollar-sign> single-quote sequence, it **must** affect quoting for subsequent characters up to the end of the quoted text. The rules for quoting are as described in 2.2 Quoting. During token recognition no substitutions **must** be actually performed, and the result token **must** contain exactly the characters that appear in the input unmodified, including any embedded or enclosing quotes or substitution operators, between the start and the end of the quoted text. The token **must** not be delimited by the end of the quoted field. |
| `SHALL-19-03-008` | Verify that: If the current character is an unquoted '$' or '`', the shell **must** identify the start of any candidates for parameter expansion ( 2.6.2 Parameter Expansion), command substitution ( 2.6.3 Command Substitution), or arithmetic expansion ( 2.6.4 Arithmetic Expansion) from their introductory unquoted character sequences: '$' or "${", "$(" or '`', and "$((", respectively. The shell **must** read sufficient input to determine the end of the unit to be expanded (as explained in the cited sections). While processing the characters, if instances of expansions or quoting are found nested within the substitution, the shell **must** recursively process them in the manner specified for the construct that is found. For "$(" and '`' only, if instances of io_here tokens are found nested within the substitution, they **must** be parsed according to the rules of 2.7.4 Here-Document; if the terminating ')' or '`' of the substitution occurs before the NEWLINE token marking the start of the here-document, the behavior is unspecified. The characters found from the beginning of the substitution to its end, allowing for any recursion necessary to recognize embedded constructs, **must** be included unmodified in the result token, including any embedded or enclosing substitution operators or quotes. The token **must** not be delimited by the end of the substitution. |
| `SHALL-19-03-009` | Verify that: If the current character is not quoted and can be used as the first character of a new operator, the current token (if any) **must** be delimited. The current character **must** be used as the beginning of the next (operator) token. |
| `SHALL-19-03-010` | Verify that: If the current character is an unquoted <blank>, any token containing the previous character is delimited and the current character **must** be discarded. |
| `SHALL-19-03-011` | Verify that: If the previous character was part of a word, the current character **must** be appended to that word. |
| `SHALL-19-03-012` | Verify that: If the current character is a '#', it and all subsequent characters up to, but excluding, the next <newline> **must** be discarded as a comment. The <newline> that ends the line is not considered part of the comment. |
| `SHALL-19-03-013` | Verify that: In situations where the shell parses its input as a program, once a complete_command has been recognized by the grammar (see 2.10 Shell Grammar), the complete_command **must** be executed before the next complete_command is tokenized and parsed. |

### 2. Shell Command Language > Token Recognition > Alias Substitution

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-03-01-001` | Verify that: After a token has been categorized as type TOKEN (see 2.10.1 Shell Grammar Lexical Conventions), including (recursively) any token resulting from an alias substitution, the TOKEN **must** be subject to alias substitution if all of the following conditions are true: |
| `SHALL-19-03-01-002` | Verify that: When a TOKEN is subject to alias substitution, the value of the alias **must** be processed as if it had been read from the input instead of the TOKEN, with token recognition (see 2.3 Token Recognition) resuming at the start of the alias value. When the end of the alias value is reached, the shell may behave as if an additional <space> character had been read from the input after the TOKEN that was replaced. If it does not add this <space>, it is unspecified whether the current token is delimited before token recognition is applied to the character (if any) that followed the TOKEN in the input. |
| `SHALL-19-03-01-003` | Verify that: An implementation may defer the effect of a change to an alias but the change **must** take effect no later than the completion of the currently executing complete_command (see 2.10 Shell Grammar). Changes to aliases **must** not take effect out of order. Implementations may provide predefined aliases that are in effect when the shell is invoked. |

### 2. Shell Command Language > Reserved Words

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-04-001` | Verify that: Reserved words are words that have special meaning to the shell; see 2.9 Shell Commands. The following words **must** be recognized as reserved words: |
| `SHALL-19-04-002` | Verify that: This recognition **must** only occur when none of the characters is quoted and when the word is used as: |
| `SHALL-19-04-003` | Verify that: When the word time is recognized as a reserved word in circumstances where it would, if it were not a reserved word, be the command name (see 2.9.1.1 Order of Processing) of a simple command that would execute the time utility in a manner other than one for which time states that the results are unspecified, the behavior **must** be as specified for the time utility. |

### 2. Shell Command Language > Parameters and Variables > Positional Parameters

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-05-01-001` | Verify that: A positional parameter is a parameter denoted by a decimal representation of a positive integer. The digits denoting the positional parameters **must** always be interpreted as a decimal value, even if there is a leading zero. When a positional parameter with more than one digit is specified, the application **must** enclose the digits in braces (see 2.6.2 Parameter Expansion). |

### 2. Shell Command Language > Parameters and Variables > Special Parameters

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-05-02-001` | Verify that: Listed below are the special parameters and the values to which they **must** expand. Only the values of the special parameters are listed; see 2.6 Word Expansions for a detailed summary of all the stages involved in expanding words. |
| `SHALL-19-05-02-002` | Verify that: If one of these conditions is true, the initial fields **must** be retained as separate fields, except that if the parameter being expanded was embedded within a word, the first field **must** be joined with the beginning part of the original word and the last field **must** be joined with the end part of the original word. In all other contexts the results of the expansion are unspecified. If there are no positional parameters, the expansion of '@' **must** generate zero fields, even when '@' is within double-quotes; however, if the expansion is embedded within a word which contains one or more other parts that expand to a quoted null string, these null string(s) **must** still produce an empty field, except that if the other parts are all within the same double-quotes as the '@', it is unspecified whether the result is zero fields or one empty field. |
| `SHALL-19-05-02-003` | Verify that: Expands to the positional parameters, starting from one, initially producing one field for each positional parameter that is set. When the expansion occurs in a context where field splitting will be performed, any empty fields may be discarded and each of the non-empty fields **must** be further split as described in 2.6.5 Field Splitting. When the expansion occurs in a context where field splitting will not be performed, the initial fields **must** be joined to form a single field with the value of each parameter separated by the first character of the IFS variable if IFS contains at least one character, or separated by a <space> if IFS is unset, or with no separation if IFS is set to a null string. |
| `SHALL-19-05-02-004` | Verify that: Expands to the shortest representation of the decimal number of positional parameters. The command name (parameter 0) **must** not be counted in the number given by '#' because it is a special parameter, not a positional parameter. |
| `SHALL-19-05-02-005` | Verify that: (Hyphen.) Expands to the current option flags (the single-letter option names concatenated into a string) as specified on invocation, by the set special built-in command, or implicitly by the shell. It is unspecified whether the -c and -s options are included in the expansion of "$-". The -i option **must** be included in "$-" if the shell is interactive, regardless of whether it was specified on invocation. |
| `SHALL-19-05-02-006` | Verify that: Expands to the shortest representation of the decimal process ID of the invoked shell. In a subshell (see 2.13 Shell Execution Environment), '$' **must** expand to the same value as that of the current shell. |

### 2. Shell Command Language > Parameters and Variables > Shell Variables

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-05-03-001` | Verify that: Variables **must** be initialized from the environment (as defined by XBD 8. Environment Variables and the exec function in the System Interfaces volume of POSIX.1-2024) and can be given new values with variable assignment commands. Shell variables **must** be initialized only from environment variables that have valid names. If a variable is initialized from the environment, it **must** be marked for export immediately; see the export special built-in. New variables can be defined and initialized with variable assignments, with the read or getopts utilities, with the name parameter in a for loop, with the ${name=word} expansion, or with other mechanisms provided as implementation extensions. |
| `SHALL-19-05-03-002` | Verify that: The following variables **must** affect the execution of the shell: |
| `SHALL-19-05-03-003` | Verify that: This variable, when and only when an interactive shell is invoked, **must** be subjected to parameter expansion (see 2.6.2 Parameter Expansion) by the shell and the resulting value **must** be used as a pathname of a file. Before any interactive commands are read, the shell **must** tokenize (see 2.3 Token Recognition) the contents of the file, parse the tokens as a program (see 2.10 Shell Grammar), and execute the resulting commands in the current environment. (In other words, the contents of the ENV file are not parsed as a single compound_list. This distinction matters because it influences when aliases take effect.) The file need not be executable. If the expanded value of ENV is not an absolute pathname, the results are unspecified. ENV **must** be ignored if the user's real and effective user IDs or real and effective group IDs are different. |
| `SHALL-19-05-03-004` | Verify that: If IFS is not set, it **must** behave as normal for an unset variable, except that field splitting by the shell and line splitting by the read utility **must** be performed as if the value of IFS is <space><tab><newline>; see 2.6.5 Field Splitting. |
| `SHALL-19-05-03-005` | Verify that: The shell **must** set IFS to <space><tab><newline> when it is invoked. |
| `SHALL-19-05-03-006` | Verify that: Determine the interpretation of sequences of bytes of text data as characters (for example, single-byte as opposed to multi-byte characters), which characters are defined as letters (character class alpha) and <blank> characters (character class blank), and the behavior of character classes within pattern matching. Changing the value of LC_CTYPE after the shell has started **must** not affect the lexical processing of shell commands in the current shell execution environment or its subshells. Invoking a shell script or performing exec sh subjects the new shell to the changes in LC_CTYPE . |
| `SHALL-19-05-03-007` | Verify that: Set by the shell to the decimal value of its parent process ID during initialization of the shell. In a subshell (see 2.13 Shell Execution Environment), PPID **must** be set to the same value as that of the parent of the current shell. For example, echo $PPID and (echo $PPID ) would produce the same value. |
| `SHALL-19-05-03-008` | Verify that: Each time an interactive shell is ready to read a command, the value of this variable **must** be subjected to parameter expansion (see 2.6.2 Parameter Expansion) and exclamation-mark expansion (see below). Whether the value is also subjected to command substitution (see 2.6.3 Command Substitution) or arithmetic expansion (see 2.6.4 Arithmetic Expansion) or both is unspecified. After expansion, the value **must** be written to standard error. |
| `SHALL-19-05-03-009` | Verify that: The expansions **must** be performed in two passes, where the result of the first pass is input to the second pass. One of the passes **must** perform only the exclamation-mark expansion described below. The other pass **must** perform the other expansion(s) according to the rules in 2.6 Word Expansions. Which of the two passes is performed first is unspecified. |
| `SHALL-19-05-03-010` | Verify that: The default value **must** be "$ ". For users who have specific additional implementation-defined privileges, the default may be another, implementation-defined value. |
| `SHALL-19-05-03-011` | Verify that: Exclamation-mark expansion: The shell **must** replace each instance of the <exclamation-mark> character ('!') with the history file number (see Command History List) of the next command to be typed. An <exclamation-mark> character escaped by another <exclamation-mark> character (that is, "!!") **must** expand to a single <exclamation-mark> character. |
| `SHALL-19-05-03-012` | Verify that: Each time the user enters a <newline> prior to completing a command line in an interactive shell, the value of this variable **must** be subjected to parameter expansion (see 2.6.2 Parameter Expansion). Whether the value is also subjected to command substitution (see 2.6.3 Command Substitution) or arithmetic expansion (see 2.6.4 Arithmetic Expansion) or both is unspecified. After expansion, the value **must** be written to standard error. The default value **must** be "> ". |
| `SHALL-19-05-03-013` | Verify that: When an execution trace (set -x) is being performed, before each line in the execution trace, the value of this variable **must** be subjected to parameter expansion (see 2.6.2 Parameter Expansion). Whether the value is also subjected to command substitution (see 2.6.3 Command Substitution) or arithmetic expansion (see 2.6.4 Arithmetic Expansion) or both is unspecified. After expansion, the value **must** be written to standard error. The default value **must** be "+ ". |
| `SHALL-19-05-03-014` | Verify that: Set by the shell and by the cd utility. In the shell the value **must** be initialized from the environment as follows. If a value for PWD is passed to the shell in the environment when it is executed, the value is an absolute pathname of the current working directory that is no longer than {PATH_MAX} bytes including the terminating null byte, and the value does not contain any components that are dot or dot-dot, then the shell **must** set PWD to the value from the environment. Otherwise, if a value for PWD is passed to the shell in the environment when it is executed, the value is an absolute pathname of the current working directory, and the value does not contain any components that are dot or dot-dot, then it is unspecified whether the shell sets PWD to the value from the environment or sets PWD to the pathname that would be output by pwd -P. Otherwise, the sh utility sets PWD to the pathname that would be output by pwd -P. In cases where PWD is set to the value from the environment, the value can contain components that refer to files of type symbolic link. In cases where PWD is set to the pathname that would be output by pwd -P, if there is insufficient permission on the current working directory, or on any parent of that directory, to determine what that pathname would be, the value of PWD is unspecified. Assignments to this variable may be ignored. If an application sets or unsets the value of PWD , the behaviors of the cd and pwd utilities are unspecified. |

### 2. Shell Command Language > Word Expansions

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-06-001` | Verify that: Tilde expansion (see 2.6.1 Tilde Expansion), parameter expansion (see 2.6.2 Parameter Expansion), command substitution (see 2.6.3 Command Substitution ), and arithmetic expansion (see 2.6.4 Arithmetic Expansion) **must** be performed, beginning to end. See item 5 in 2.3 Token Recognition. |
| `SHALL-19-06-002` | Verify that: Field splitting (see 2.6.5 Field Splitting) **must** be performed on the portions of the fields generated by step 1. |
| `SHALL-19-06-003` | Verify that: Pathname expansion (see 2.6.6 Pathname Expansion) **must** be performed, unless set -f is in effect. |
| `SHALL-19-06-004` | Verify that: Quote removal (see 2.6.7 Quote Removal), if performed, **must** always be performed last. |
| `SHALL-19-06-005` | Verify that: Tilde expansions, parameter expansions, command substitutions, arithmetic expansions, and quote removals that occur within a single word **must** expand to a single field, except as described below. The shell **must** create multiple fields or no fields from a single word only as a result of field splitting, pathname expansion, or the following cases: |
| `SHALL-19-06-006` | Verify that: may be subject to an additional implementation-defined form of expansion that can create multiple fields from a single word. This expansion, if supported, **must** be applied before all the other word expansions are applied. The other expansions **must** then be applied to each field that results from this expansion. |
| `SHALL-19-06-007` | Verify that: When expanding words for a command about to be executed, and the word will be the command name or an argument to the command, the expansions **must** be carried out in the current shell execution environment. (The environment for the command to be executed is unknown until the command word is known.) |
| `SHALL-19-06-008` | Verify that: the result is unspecified. If a '$' that is neither within single-quotes nor escaped by a <backslash> is immediately followed by a <space>, <tab>, or a <newline>, or is not followed by any character, the '$' **must** be treated as a literal character. |

### 2. Shell Command Language > Word Expansions > Tilde Expansion

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-06-01-001` | Verify that: If the tilde-prefix consists of only the <tilde> character, it **must** be replaced by the value of the variable HOME . If HOME is unset, the results are unspecified. |
| `SHALL-19-06-01-002` | Verify that: Otherwise, the characters in the tilde-prefix following the <tilde> **must** be treated as a possible login name from the user database. If these characters do not form a portable login name (see the description of the LOGNAME environment variable in XBD 8.3 Other Environment Variables), the results are unspecified. |
| `SHALL-19-06-01-003` | Verify that: If the characters in the tilde-prefix following the <tilde> form a portable login name, the tilde-prefix **must** be replaced by a pathname of the initial working directory associated with the login name. The pathname **must** be obtained as if by using the getpwnam() function as defined in the System Interfaces volume of POSIX.1-2024. If the system does not recognize the login name, the results are unspecified. |
| `SHALL-19-06-01-004` | Verify that: The pathname that replaces the tilde-prefix **must** be treated as if quoted to prevent it being altered by field splitting and pathname expansion; if a <slash> follows the tilde-prefix and the pathname ends with a <slash>, the trailing <slash> from the pathname should be omitted from the replacement. If the word being expanded consists of only the <tilde> character and HOME is set to the null string, this produces an empty field (as opposed to zero fields) as the expanded word. |

### 2. Shell Command Language > Word Expansions > Parameter Expansion

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-06-02-001` | Verify that: where expression consists of all characters until the matching '}'. Any '}' escaped by a <backslash> or within a quoted string, and characters in embedded arithmetic expansions, command substitutions, and variable expansions, **must** not be examined in determining the matching '}'. |
| `SHALL-19-06-02-002` | Verify that: The value, if any, of parameter **must** be substituted. |
| `SHALL-19-06-02-003` | Verify that: If the parameter is a name, the expansion **must** use the longest valid name (see XBD 3.216 Name), whether or not the variable denoted by that name exists. |
| `SHALL-19-06-02-004` | Verify that: Use Default Values. If parameter is unset or null, the expansion of word (or an empty string if word is omitted) **must** be substituted; otherwise, the value of parameter **must** be substituted. |
| `SHALL-19-06-02-005` | Verify that: Assign Default Values. If parameter is unset or null, quote removal **must** be performed on the expansion of word and the result (or an empty string if word is omitted) **must** be assigned to parameter. In all cases, the final value of parameter **must** be substituted. Only variables, not positional parameters or special parameters, can be assigned in this way. |
| `SHALL-19-06-02-006` | Verify that: Indicate Error if Null or Unset. If parameter is unset or null, the expansion of word (or a message indicating it is unset if word is omitted) **must** be written to standard error and the shell exits with a non-zero exit status. Otherwise, the value of parameter **must** be substituted. An interactive shell need not exit. |
| `SHALL-19-06-02-007` | Verify that: Use Alternative Value. If parameter is unset or null, null **must** be substituted; otherwise, the expansion of word (or an empty string if word is omitted) **must** be substituted. |
| `SHALL-19-06-02-008` | Verify that: In the parameter expansions shown previously, use of the <colon> in the format **must** result in a test for a parameter that is unset or null; omission of the <colon> **must** result in a test for a parameter that is only unset. If parameter is '#' and the colon is omitted, the application **must** ensure that word is specified (this is necessary to avoid ambiguity with the string length expansion). The following table summarizes the effect of the <colon>: |
| `SHALL-19-06-02-009` | Verify that: String Length. The shortest decimal representation of the length in characters of the value of parameter **must** be substituted. If parameter is '*' or '@', the result of the expansion is unspecified. If parameter is unset and set -u is in effect, the expansion **must** fail. |
| `SHALL-19-06-02-010` | Verify that: The following four varieties of parameter expansion provide for character substring processing. In each case, pattern matching notation (see 2.14 Pattern Matching Notation), rather than regular expression notation, **must** be used to evaluate the patterns. If parameter is '#', '*', or '@', the result of the expansion is unspecified. If parameter is unset and set -u is in effect, the expansion **must** fail. Enclosing the full parameter expansion string in double-quotes **must** not cause the following four varieties of pattern characters to be quoted, whereas quoting characters within the braces **must** have this effect. In each variety, if word is omitted, the empty pattern **must** be used. |
| `SHALL-19-06-02-011` | Verify that: Remove Smallest Suffix Pattern. The word **must** be expanded to produce a pattern. The parameter expansion **must** then result in parameter, with the smallest portion of the suffix matched by the pattern deleted. If present, word **must** not begin with an unquoted '%'. |
| `SHALL-19-06-02-012` | Verify that: Remove Largest Suffix Pattern. The word **must** be expanded to produce a pattern. The parameter expansion **must** then result in parameter, with the largest portion of the suffix matched by the pattern deleted. |
| `SHALL-19-06-02-013` | Verify that: Remove Smallest Prefix Pattern. The word **must** be expanded to produce a pattern. The parameter expansion **must** then result in parameter, with the smallest portion of the prefix matched by the pattern deleted. If present, word **must** not begin with an unquoted '#'. |
| `SHALL-19-06-02-014` | Verify that: Remove Largest Prefix Pattern. The word **must** be expanded to produce a pattern. The parameter expansion **must** then result in parameter, with the largest portion of the prefix matched by the pattern deleted. |

### 2. Shell Command Language > Word Expansions > Command Substitution

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-06-03-001` | Verify that: Command substitution allows the output of one or more commands to be substituted in place of the commands themselves. Command substitution **must** occur when command(s) are enclosed as follows: |
| `SHALL-19-06-03-002` | Verify that: Within the backquoted style of command substitution, if the command substitution is not within double-quotes, <backslash> **must** retain its literal meaning, except when followed by: '$', '`', or <backslash>. See 2.2.3 Double-Quotes for the handling of <backslash> when the command substitution is within double-quotes. The search for the matching backquote **must** be satisfied by the first unquoted non-escaped backquote; during this search, if a non-escaped backquote is encountered within a shell comment, a here-document, an embedded command substitution of the $(commands) form, or a quoted string, undefined results occur. A quoted string that begins, but does not end, within the "`...`" sequence produces undefined results. |
| `SHALL-19-06-03-003` | Verify that: With both the backquoted and $(commands) forms, the commands string **must** be tokenized (see 2.3 Token Recognition) and parsed (see 2.10 Shell Grammar). It is unspecified whether the commands string is parsed and executed incrementally as a program (as for a shell script), or is parsed as a single compound_list that is executed after the string has been completely parsed. In addition, it is unspecified whether the terminating ')' of the $(commands) form can result from alias substitution. With the $(commands) form any syntactically correct program can be used for commands, except that: |
| `SHALL-19-06-03-004` | Verify that: If the commands string is parsed as a single compound_list, before any commands are executed, alias and unalias commands in commands have no effect during parsing (see 2.3.1 Alias Substitution). Strictly conforming applications **must** ensure that the commands string does not depend on alias changes taking effect incrementally as would be the case if parsed and executed as a program. |
| `SHALL-19-06-03-005` | Verify that: The results of command substitution **must** not be processed for further tilde expansion, parameter expansion, command substitution, or arithmetic expansion. |
| `SHALL-19-06-03-006` | Verify that: Command substitution can be nested. To specify nesting within the backquoted version, the application **must** precede the inner backquotes with <backslash> characters; for example: |
| `SHALL-19-06-03-007` | Verify that: The syntax of the shell command language has an ambiguity for expansions beginning with "$((", which can introduce an arithmetic expansion or a command substitution that starts with a subshell. Arithmetic expansion has precedence; that is, the shell **must** first determine whether it can parse the expansion as an arithmetic expansion and **must** only parse the expansion as a command substitution if it determines that it cannot parse the expansion as an arithmetic expansion. The shell need not evaluate nested expansions when performing this determination. If it encounters the end of input without already having determined that it cannot parse the expansion as an arithmetic expansion, the shell **must** treat the expansion as an incomplete arithmetic expansion and report a syntax error. A conforming application **must** ensure that it separates the "$(" and '(' into two tokens (that is, separate them with white space) in a command substitution that starts with a subshell. For example, a command substitution containing a single subshell could be written as: |

### 2. Shell Command Language > Word Expansions > Arithmetic Expansion

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-06-04-001` | Verify that: Arithmetic expansion provides a mechanism for evaluating an arithmetic expression and substituting its value. The format for arithmetic expansion **must** be as follows: |
| `SHALL-19-06-04-002` | Verify that: The expression **must** be treated as if it were in double-quotes, except that a double-quote inside the expression is not treated specially. The shell **must** expand all tokens in the expression for parameter expansion, command substitution, and quote removal. |
| `SHALL-19-06-04-003` | Verify that: Next, the shell **must** treat this as an arithmetic expression and substitute the value of the expression. The arithmetic expression **must** be processed according to the rules given in 1.1.2.1 Arithmetic Precision and Operations, with the following exceptions: |
| `SHALL-19-06-04-004` | Verify that: All changes to variables in an arithmetic expression **must** be in effect after the arithmetic expansion, as in the parameter expansion "${x=value}". |
| `SHALL-19-06-04-005` | Verify that: If the shell variable x contains a value that forms a valid integer constant, optionally including a leading <plus-sign> or <hyphen-minus>, then the arithmetic expansions "$((x))" and "$(($x))" **must** return the same value. |
| `SHALL-19-06-04-006` | Verify that: As an extension, the shell may recognize arithmetic expressions beyond those listed. The shell may use a signed integer type with a rank larger than the rank of signed long. The shell may use a real-floating type instead of signed long as long as it does not affect the results in cases where there is no overflow. If the expression is invalid, or the contents of a shell variable used in the expression are not recognized by the shell, the expansion fails and the shell **must** write a diagnostic message to standard error indicating the failure. |

### 2. Shell Command Language > Word Expansions > Field Splitting

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-06-05-001` | Verify that: After parameter expansion ( 2.6.2 Parameter Expansion), command substitution ( 2.6.3 Command Substitution), and arithmetic expansion ( 2.6.4 Arithmetic Expansion), if the shell variable IFS (see 2.5.3 Shell Variables) is set and its value is not empty, or if IFS is unset, the shell **must** scan each field containing results of expansions and substitutions that did not occur in double-quotes for field splitting; zero, one or multiple fields can result. |
| `SHALL-19-06-05-002` | Verify that: If the IFS variable is set and has an empty string as its value, no field splitting **must** occur. However, if an input field which contained the results of an expansion is entirely empty, it **must** be removed. Note that this occurs before quote removal; any input field that contains any quoting characters can never be empty at this point. After the removal of any such fields from the input, the possibly modified input field list **must** become the output. |
| `SHALL-19-06-05-003` | Verify that: Fields which contain no results from expansions **must** not be affected by field splitting, and **must** remain unaltered, simply moving from the list of input fields to be next in the list of output fields. |
| `SHALL-19-06-05-004` | Verify that: The shell **must** use the byte sequences that form the characters in the value of the IFS variable as delimiters. Each of the characters <space>, <tab>, and <newline> which appears in the value of IFS **must** be a single-byte delimiter. The shell **must** use these delimiters as field terminators to split the results of expansions, along with other adjacent bytes, into separate fields, as described below. Note that these delimiters terminate a field; they do not, of themselves, cause a new field to start—subsequent bytes that are not from the results of an expansion, or that do not form IFS white-space characters are required for a new field to begin. |
| `SHALL-19-06-05-005` | Verify that: If the results of the algorithm are that no fields are delimited; that is, if the input field is wholly empty or consists entirely of IFS white space, the result **must** be zero fields (rather than an empty field). |
| `SHALL-19-06-05-006` | Verify that: Each field containing the results from an expansion **must** be processed in order, intermixed with fields not containing the results of expansions, processed as described above, as if by using the following algorithm, examining bytes in the input field, from beginning to end: |
| `SHALL-19-06-05-007` | Verify that: At this point, if the candidate is not empty, or if a sequence of bytes representing an IFS character that is not IFS white space was seen at step 4, then a field is said to have been delimited, and the candidate **must** become an output field. |
| `SHALL-19-06-05-008` | Verify that: Once the input is empty, the candidate **must** become an output field if and only if it is not empty. |
| `SHALL-19-06-05-009` | Verify that: The ordered list of output fields so produced, which might be empty, **must** replace the list of input fields. |

### 2. Shell Command Language > Word Expansions > Pathname Expansion

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-06-06-001` | Verify that: After field splitting, if set -f is not in effect, each field in the resulting command line **must** be expanded using the algorithm described in 2.14 Pattern Matching Notation, qualified by the rules in 2.14.3 Patterns Used for Filename Expansion. |

### 2. Shell Command Language > Word Expansions > Quote Removal

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-06-07-001` | Verify that: The quote character sequence <dollar-sign> single-quote and the single-character quote characters (<backslash>, single-quote, and double-quote) that were present in the original word **must** be removed unless they have themselves been quoted. Note that the single-quote character that terminates a <dollar-sign> single-quote sequence is itself a single-character quote character. |

### 2. Shell Command Language > Redirection

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-07-001` | Verify that: The number n is an optional one or more digit decimal number designating the file descriptor number; the application **must** ensure it is delimited from any preceding text and immediately precedes the redirection operator redir-op (with no intervening <blank> characters allowed). If n is quoted, the number **must** not be recognized as part of the redirection expression. For example: |
| `SHALL-19-07-002` | Verify that: writes the characters 2>a to standard output. The optional number, redirection operator, and word **must** not appear in the arguments provided to the command to be executed (if any). |
| `SHALL-19-07-003` | Verify that: The largest file descriptor number supported in shell redirections is implementation-defined; however, all implementations **must** support at least 0 to 9, inclusive, for use by the application. |
| `SHALL-19-07-004` | Verify that: If the redirection operator is "<<" or "<<-", the word that follows the redirection operator **must** be subjected to quote removal; it is unspecified whether any of the other expansions occur. For the other redirection operators, the word that follows the redirection operator **must** be subjected to tilde expansion, parameter expansion, command substitution, arithmetic expansion, and quote removal. Pathname expansion **must** not be performed on the word by a non-interactive shell; an interactive shell may perform it, but if the expansion would result in more than one word it is unspecified whether the redirection proceeds without pathname expansion being performed or the redirection fails. |
| `SHALL-19-07-005` | Verify that: A failure to open or create a file **must** cause a redirection to fail. |

### 2. Shell Command Language > Redirection > Redirecting Input

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-07-01-001` | Verify that: Input redirection **must** cause the file whose name results from the expansion of word to be opened for reading on the designated file descriptor, or standard input if the file descriptor is not specified. |
| `SHALL-19-07-01-002` | Verify that: where the optional n represents the file descriptor number. If the number is omitted, the redirection **must** refer to standard input (file descriptor 0). |

### 2. Shell Command Language > Redirection > Redirecting Output

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-07-02-001` | Verify that: where the optional n represents the file descriptor number. If the number is omitted, the redirection **must** refer to standard output (file descriptor 1). |
| `SHALL-19-07-02-002` | Verify that: Output redirection using the '>' format **must** fail if the noclobber option is set (see the description of set -C) and the file named by the expansion of word exists and is either a regular file or a symbolic link that resolves to a regular file; it may also fail if the file is a symbolic link that does not resolve to an existing file. The check for existence, file creation, and open operations **must** be performed atomically as is done by the open() function as defined in System Interfaces volume of POSIX.1-2024 when the O_CREAT and O_EXCL flags are set, except that if the file exists and is a symbolic link, the open operation need not fail with [EEXIST] unless the symbolic link resolves to an existing regular file. Performing these operations atomically ensures that the creation of lock files and unique (often temporary) files is reliable, with important caveats detailed in C.2.7.2 Redirecting Output. The check for the type of the file need not be performed atomically with the check for existence, file creation, and open operations. If not, there is a potential race condition that may result in a misleading shell diagnostic message when redirection fails. See XRAT C.2.7.2 Redirecting Output for more details. |
| `SHALL-19-07-02-003` | Verify that: In all other cases (noclobber not set, redirection using '>' does not fail for the reasons stated above, or redirection using the ">\|" format), output redirection **must** cause the file whose name results from the expansion of word to be opened for output on the designated file descriptor, or standard output if none is specified. If the file does not exist, it **must** be created as an empty file; otherwise, it **must** be opened as if the open() function was called with the O_TRUNC flag set. |

### 2. Shell Command Language > Redirection > Appending Redirected Output

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-07-03-001` | Verify that: Appended output redirection **must** cause the file whose name results from the expansion of word to be opened for output on the designated file descriptor. The file **must** be opened as if the open() function as defined in the System Interfaces volume of POSIX.1-2024 was called with the O_APPEND flag set. If the file does not exist, it **must** be created. |

### 2. Shell Command Language > Redirection > Here-Document

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-07-04-001` | Verify that: The here-document **must** be treated as a single word that begins after the next NEWLINE token and continues until there is a line containing only the delimiter and a <newline>, with no <blank> characters in between. Then the next here-document starts, if there is one. For the purposes of locating this terminating line, the end of a command_string operand (see sh) **must** be treated as a <newline> character, and the end of the commands string in $(commands) and `commands` may be treated as a <newline>. If the end of input is reached without finding the terminating line, the shell should, but need not, treat this as a redirection error. The format is as follows: |
| `SHALL-19-07-04-002` | Verify that: If any part of word is quoted, not counting double-quotes outside a command substitution if the here-document is inside one, the delimiter **must** be formed by performing quote removal on word, and the here-document lines **must** not be expanded. Otherwise: |
| `SHALL-19-07-04-003` | Verify that: The delimiter **must** be the word itself. |
| `SHALL-19-07-04-004` | Verify that: The removal of <backslash><newline> for line continuation (see 2.2.1 Escape Character (Backslash)) **must** be performed during the search for the trailing delimiter. (As a consequence, the trailing delimiter is not recognized immediately after a <newline> that was removed by line continuation.) It is unspecified whether the line containing the trailing delimiter is itself subject to this line continuation. |
| `SHALL-19-07-04-005` | Verify that: All lines of the here-document **must** be expanded, when the redirection operator is evaluated but after the trailing delimiter for the here-document has been located, for parameter expansion, command substitution, and arithmetic expansion. If the redirection operator is never evaluated (because the command it is part of is not executed), the here-document **must** be read without performing any expansions. |
| `SHALL-19-07-04-006` | Verify that: Any <backslash> characters in the input **must** behave as the <backslash> inside double-quotes (see 2.2.3 Double-Quotes). However, the double-quote character ('"') **must** not be treated specially within a here-document, except when the double-quote appears within "$()", "``", or "${}". |
| `SHALL-19-07-04-007` | Verify that: If the redirection operator is "<<-", all leading <tab> characters **must** be stripped from input lines after <backslash><newline> line continuation (when it applies) has been performed, and from the line containing the trailing delimiter. Stripping of leading <tab> characters **must** occur as the here-document is read from the shell input (and consequently does not affect any <tab> characters that result from expansions). |
| `SHALL-19-07-04-008` | Verify that: If more than one "<<" or "<<-" operator is specified on a line, the here-document associated with the first operator **must** be supplied first by the application and **must** be read first by the shell. |
| `SHALL-19-07-04-009` | Verify that: When a here-document is read from a terminal device and the shell is interactive, it **must** write the contents of the variable PS2, processed as described in 2.5.3 Shell Variables, to standard error before reading each line of input until the delimiter has been recognized. |

### 2. Shell Command Language > Redirection > Duplicating an Input File Descriptor

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-07-05-001` | Verify that: **must** duplicate one input file descriptor from another, or **must** close one. If word evaluates to one or more digits, the file descriptor denoted by n, or standard input if n is not specified, **must** be made to be a copy of the file descriptor denoted by word; if the digits in word do not represent an already open file descriptor, a redirection error **must** result (see 2.8.1 Consequences of Shell Errors); if the file descriptor denoted by word represents an open file descriptor that is not open for input, a redirection error may result. If word evaluates to '-', file descriptor n, or standard input if n is not specified, **must** be closed. Attempts to close a file descriptor that is not open **must** not constitute an error. If word evaluates to something else, the behavior is unspecified. |

### 2. Shell Command Language > Redirection > Duplicating an Output File Descriptor

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-07-06-001` | Verify that: **must** duplicate one output file descriptor from another, or **must** close one. If word evaluates to one or more digits, the file descriptor denoted by n, or standard output if n is not specified, **must** be made to be a copy of the file descriptor denoted by word; if the digits in word do not represent an already open file descriptor, a redirection error **must** result (see 2.8.1 Consequences of Shell Errors); if the file descriptor denoted by word represents an open file descriptor that is not open for output, a redirection error may result. If word evaluates to '-', file descriptor n, or standard output if n is not specified, is closed. Attempts to close a file descriptor that is not open **must** not constitute an error. If word evaluates to something else, the behavior is unspecified. |

### 2. Shell Command Language > Redirection > Open File Descriptors for Reading and Writing

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-07-07-001` | Verify that: **must** cause the file whose name is the expansion of word to be opened for both reading and writing on the file descriptor denoted by n, or standard input if n is not specified. If the file does not exist, it **must** be created. |

### 2. Shell Command Language > Exit Status and Errors > Consequences of Shell Errors

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-08-01-001` | Verify that: Certain errors **must** cause the shell to write a diagnostic message to standard error and exit as shown in the following table: |
| `SHALL-19-08-01-002` | Verify that: **must** exit |
| `SHALL-19-08-01-003` | Verify that: **must** not exit |
| `SHALL-19-08-01-004` | Verify that: **must** exit1 |
| `SHALL-19-08-01-005` | Verify that: **must** not exit |
| `SHALL-19-08-01-006` | Verify that: **must** not exit |
| `SHALL-19-08-01-007` | Verify that: **must** not exit |
| `SHALL-19-08-01-008` | Verify that: **must** exit |
| `SHALL-19-08-01-009` | Verify that: **must** not exit |
| `SHALL-19-08-01-010` | Verify that: **must** not exit |
| `SHALL-19-08-01-011` | Verify that: **must** not exit |
| `SHALL-19-08-01-012` | Verify that: **must** not exit |
| `SHALL-19-08-01-013` | Verify that: **must** not exit |
| `SHALL-19-08-01-014` | Verify that: **must** not exit |
| `SHALL-19-08-01-015` | Verify that: **must** not exit |
| `SHALL-19-08-01-016` | Verify that: **must** exit |
| `SHALL-19-08-01-017` | Verify that: **must** not exit |
| `SHALL-19-08-01-018` | Verify that: **must** exit |
| `SHALL-19-08-01-019` | Verify that: **must** not exit |
| `SHALL-19-08-01-020` | Verify that: **must** not exit |
| `SHALL-19-08-01-021` | Verify that: **must** exit4 |
| `SHALL-19-08-01-022` | Verify that: **must** exit4 |
| `SHALL-19-08-01-023` | Verify that: The shell **must** exit only if the special built-in utility is executed directly. If it is executed via the command utility, the shell **must** not exit. |
| `SHALL-19-08-01-024` | Verify that: The shell is not required to write a diagnostic message, but the utility itself **must** write a diagnostic message if required to do so. |
| `SHALL-19-08-01-025` | Verify that: If an unrecoverable read error occurs when reading commands, other than from the file operand of the dot special built-in, the shell **must** execute no further commands (including any already successfully read but not yet executed) other than any specified in a previously defined EXIT trap action. An unrecoverable read error while reading from the file operand of the dot special built-in **must** be treated as a special built-in utility error. |
| `SHALL-19-08-01-026` | Verify that: If any of the errors shown as "**must** exit" or "may exit" occur in a subshell environment, the shell **must** (respectively, may) exit from the subshell environment with a non-zero status and continue in the environment from which that subshell environment was invoked. |
| `SHALL-19-08-01-027` | Verify that: In all of the cases shown in the table where an interactive shell is required not to exit and a non-interactive shell is required to exit, an interactive shell **must** not perform any further processing of the command in which the error occurred. |

### 2. Shell Command Language > Exit Status and Errors > Exit Status for Commands

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-08-02-001` | Verify that: The exit status of a command **must** be determined as follows: |
| `SHALL-19-08-02-002` | Verify that: If the command is not found, the exit status **must** be 127. |
| `SHALL-19-08-02-003` | Verify that: Otherwise, if the command name is found, but it is not an executable utility, the exit status **must** be 126. |
| `SHALL-19-08-02-004` | Verify that: Otherwise, if the command terminated due to the receipt of a signal, the shell **must** assign it an exit status greater than 128. The exit status **must** identify, in an implementation-defined manner, which signal terminated the command. Note that shell implementations are permitted to assign an exit status greater than 255 if a command terminates due to a signal. |
| `SHALL-19-08-02-005` | Verify that: Otherwise, the exit status **must** be the value obtained by the equivalent of the WEXITSTATUS macro applied to the status obtained by the wait() function (as defined in the System Interfaces volume of POSIX.1-2024). Note that for C programs, this value is equal to the result of performing a modulo 256 operation on the value passed to _Exit(), _exit(), or exit() or returned from main(). |

### 2. Shell Command Language > Shell Commands

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-09-001` | Verify that: Unless otherwise stated, the exit status of a command **must** be that of the last simple command executed by the command. There **must** be no limit on the size of any shell command other than that imposed by the underlying system (memory constraints, {ARG_MAX}, and so on). |

### 2. Shell Command Language > Shell Commands > Simple Commands > Order of Processing

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-09-01-01-001` | Verify that: When a given simple command is required to be executed (that is, when any conditional construct such as an AND-OR list or a case statement has not bypassed the simple command), the following expansions, assignments, and redirections **must** all be performed from the beginning of the command text to the end: |
| `SHALL-19-09-01-01-002` | Verify that: The first word (if any) that is not a variable assignment or redirection **must** be expanded. If any fields remain following its expansion, the first field **must** be considered the command name. If no fields remain, the next word (if any) **must** be expanded, and so on, until a command name is found or no words remain. If there is a command name and it is recognized as a declaration utility, then any remaining words after the word that expanded to produce the command name, that would be recognized as a variable assignment in isolation, **must** be expanded as a variable assignment (tilde expansion after the first <equals-sign> and after any unquoted <colon>, parameter expansion, command substitution, arithmetic expansion, and quote removal, but no field splitting or pathname expansion); while remaining words that would not be a variable assignment in isolation **must** be subject to regular expansion (tilde expansion for only a leading <tilde>, parameter expansion, command substitution, arithmetic expansion, field splitting, pathname expansion, and quote removal). For all other command names, words after the word that produced the command name **must** be subject only to regular expansion. All fields resulting from the expansion of the word that produced the command name and the subsequent words, except for the field containing the command name, **must** be the arguments for the command. |
| `SHALL-19-09-01-01-003` | Verify that: Redirections **must** be performed as described in 2.7 Redirection. |
| `SHALL-19-09-01-01-004` | Verify that: Each variable assignment **must** be expanded for tilde expansion, parameter expansion, command substitution, arithmetic expansion, and quote removal prior to assigning the value. |

### 2. Shell Command Language > Shell Commands > Simple Commands > Variable Assignments

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-09-01-02-001` | Verify that: Variable assignments **must** be performed as follows: |
| `SHALL-19-09-01-02-002` | Verify that: If no command name results, variable assignments **must** affect the current execution environment. |
| `SHALL-19-09-01-02-003` | Verify that: If the command name is a standard utility implemented as a function (see XBD 4.25 Utility), the effect of variable assignments **must** be as if the utility was not implemented as a function. |
| `SHALL-19-09-01-02-004` | Verify that: If any of the variable assignments attempt to assign a value to a variable for which the readonly attribute is set in the current shell environment (regardless of whether the assignment is made in that environment), a variable assignment error **must** occur. See 2.8.1 Consequences of Shell Errors for the consequences of these errors. |

### 2. Shell Command Language > Shell Commands > Simple Commands > Commands with no Command Name

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-09-01-03-001` | Verify that: If a simple command has no command name after word expansion (see 2.9.1.1 Order of Processing), any redirections **must** be performed in a subshell environment; it is unspecified whether this subshell environment is the same one as that used for a command substitution within the command. (To affect the current execution environment, see the exec special built-in.) If any of the redirections performed in the current shell execution environment fail, the command **must** immediately fail with an exit status greater than zero, and the shell **must** write an error message indicating the failure. See 2.8.1 Consequences of Shell Errors for the consequences of these failures on interactive and non-interactive shells. |
| `SHALL-19-09-01-03-002` | Verify that: Additionally, if there is no command name but the command contains a command substitution, the command **must** complete with the exit status of the command substitution whose exit status was the last to be obtained. Otherwise, the command **must** complete with a zero exit status. |

### 2. Shell Command Language > Shell Commands > Simple Commands > Command Search and Execution

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-09-01-04-001` | Verify that: If a simple command has a command name and an optional list of arguments after word expansion (see 2.9.1.1 Order of Processing), the following actions **must** be performed: |
| `SHALL-19-09-01-04-002` | Verify that: If the command name matches the name of a special built-in utility, that special built-in utility **must** be invoked. |
| `SHALL-19-09-01-04-003` | Verify that: If the command name matches the name of a function known to this shell, the function **must** be invoked as described in 2.9.5 Function Definition Command. If the implementation has provided a standard utility in the form of a function, and that function definition still exists (i.e. has not been removed using unset -f or replaced via another function definition with the same name), it **must** not be recognized at this point. It **must** be invoked in conjunction with the path search in step 1e. |
| `SHALL-19-09-01-04-004` | Verify that: If the command name matches the name of an intrinsic utility (see 1.7 Intrinsic Utilities), that utility **must** be invoked. |
| `SHALL-19-09-01-04-005` | Verify that: If the system has implemented the utility as a built-in or as a shell function, and the built-in or function is associated with the directory that was most recently tested during the successful PATH search, that built-in or function **must** be invoked. |
| `SHALL-19-09-01-04-006` | Verify that: Otherwise, the shell **must** execute a non-built-in utility as described in 2.9.1.6 Non-built-in Utility Execution. |
| `SHALL-19-09-01-04-007` | Verify that: Once a utility has been searched for and found (either as a result of this specific search or as part of an unspecified shell start-up activity), an implementation may remember its location and need not search for the utility again unless the PATH variable has been the subject of an assignment. If the remembered location fails for a subsequent invocation, the shell **must** repeat the search to find the new location for the utility, if any. |
| `SHALL-19-09-01-04-008` | Verify that: If the search is unsuccessful, the command **must** fail with an exit status of 127 and the shell **must** write an error message. |
| `SHALL-19-09-01-04-009` | Verify that: If the command name contains at least one <slash>, the shell **must** execute a non-built-in utility as described in 2.9.1.6 Non-built-in Utility Execution. |

### 2. Shell Command Language > Shell Commands > Simple Commands > Non-built-in Utility Execution

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-09-01-06-001` | Verify that: If the execution is being made via the exec special built-in utility, the shell **must** not create a separate utility environment for this execution; the new process image **must** replace the current shell execution environment. If the current shell environment is a subshell environment, the new process image **must** replace the subshell environment and the shell **must** continue in the environment from which that subshell environment was invoked. |
| `SHALL-19-09-01-06-002` | Verify that: In either case, execution of the utility in the specified environment **must** be performed as follows: |
| `SHALL-19-09-01-06-003` | Verify that: If the execl() function fails due to an error equivalent to the [ENOEXEC] error defined in the System Interfaces volume of POSIX.1-2024, the shell **must** execute a command equivalent to having a shell invoked with the pathname resulting from the search as its first operand, with any remaining arguments passed to the new shell, except that the value of "$0" in the new shell may be set to the command name. The shell may apply a heuristic check to determine if the file to be executed could be a script and may bypass this command execution if it determines that the file cannot be a script. In this case, it **must** write an error message, and the command **must** fail with an exit status of 126. |
| `SHALL-19-09-01-06-004` | Verify that: If the search is unsuccessful, the command **must** fail with an exit status of 127 and the shell **must** write an error message. |
| `SHALL-19-09-01-06-005` | Verify that: If the execl() function fails due to an error equivalent to the [ENOEXEC] error, the shell **must** execute a command equivalent to having a shell invoked with the command name as its first operand, with any remaining arguments passed to the new shell. The shell may apply a heuristic check to determine if the file to be executed could be a script and may bypass this command execution if it determines that the file cannot be a script. In this case, it **must** write an error message, and the command **must** fail with an exit status of 126. |
| `SHALL-19-09-01-06-006` | Verify that: If the named utility does not exist, the command **must** fail with an exit status of 127 and the shell **must** write an error message. |

### 2. Shell Command Language > Shell Commands > Pipelines

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-09-02-001` | Verify that: If the pipeline begins with the reserved word ! and command1 is a subshell command, the application **must** ensure that the ( operator at the beginning of command1 is separated from the ! by one or more <blank> characters. The behavior of the reserved word ! immediately followed by the ( operator is unspecified. |
| `SHALL-19-09-02-002` | Verify that: If the pipeline is not in the background (see 2.9.3.1 Asynchronous AND-OR Lists and 2.11 Job Control), the shell **must** wait for the last command specified in the pipeline to complete, and may also wait for all commands to complete. |

### 2. Shell Command Language > Shell Commands > Pipelines > Exit Status

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-09-02-01-001` | Verify that: The exit status of a pipeline **must** depend on whether or not the pipefail option (see set) is enabled and whether or not the pipeline begins with the ! reserved word, as described in the following table. The pipefail option determines which command in the pipeline the exit status is derived from; the ! reserved word causes the exit status to be the logical NOT of the exit status of that command. The shell **must** use the pipefail setting at the time it begins execution of the pipeline, not the setting at the time it sets the exit status of the pipeline. (For example, in command1 \| set -o pipefail the exit status of command1 has no effect on the exit status of the pipeline, even if the shell executes set -o pipefail in the current shell environment.) |

### 2. Shell Command Language > Shell Commands > Lists

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-09-03-001` | Verify that: The operators "&&" and "\|\|" **must** have equal precedence and **must** be evaluated with left associativity. For example, both of the following commands write solely bar to standard output: |
| `SHALL-19-09-03-002` | Verify that: A ';' separator or a ';' or <newline> terminator **must** cause the preceding AND-OR list to be executed sequentially; an '&' separator or terminator **must** cause asynchronous execution of the preceding AND-OR list. |

### 2. Shell Command Language > Shell Commands > Lists > Asynchronous AND-OR Lists

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-09-03-02-001` | Verify that: If an AND-OR list is terminated by the control operator <ampersand> ('&'), the shell **must** execute the AND-OR list asynchronously in a subshell environment. This subshell **must** execute in the background; that is, the shell **must** not wait for the subshell to terminate before executing the next command (if any); if there are no further commands to execute, the shell **must** not wait for the subshell to terminate before exiting. |
| `SHALL-19-09-03-02-002` | Verify that: If job control is enabled (see set, -m), the AND-OR list **must** become a job-control background job and a job number **must** be assigned to it. If job control is disabled, the AND-OR list may become a non-job-control background job, in which case a job number **must** be assigned to it; if no job number is assigned it **must** become a background command but not a background job. |
| `SHALL-19-09-03-02-003` | Verify that: The process ID associated with the asynchronous AND-OR list **must** become known in the current shell execution environment; see 2.13 Shell Execution Environment. This process ID **must** remain known until any one of the following occurs (and, unless otherwise specified, may continue to remain known after it occurs). |
| `SHALL-19-09-03-02-004` | Verify that: If the shell is interactive and the asynchronous AND-OR list became a background job, the job number and the process ID associated with the job **must** be written to standard error using the format: |
| `SHALL-19-09-03-02-005` | Verify that: If the shell is interactive and the asynchronous AND-OR list did not become a background job, the process ID associated with the asynchronous AND-OR list **must** be written to standard error in an unspecified format. |

### 2. Shell Command Language > Shell Commands > Lists > Exit Status

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-09-03-03-001` | Verify that: The exit status of an asynchronous AND-OR list **must** be zero. |
| `SHALL-19-09-03-05-001` | Verify that: The exit status of a sequential AND-OR list **must** be the exit status of the last pipeline in the AND-OR list that is executed. |
| `SHALL-19-09-03-07-001` | Verify that: The exit status of an AND list **must** be the exit status of the last command that is executed in the list. |
| `SHALL-19-09-03-09-001` | Verify that: The exit status of an OR list **must** be the exit status of the last command that is executed in the list. |

### 2. Shell Command Language > Shell Commands > Lists > Sequential AND-OR Lists

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-09-03-04-001` | Verify that: AND-OR lists that are separated by a <semicolon> (';') **must** be executed sequentially. The format for executing AND-OR lists sequentially **must** be: |
| `SHALL-19-09-03-04-002` | Verify that: Each AND-OR list **must** be expanded and executed in the order specified. |
| `SHALL-19-09-03-04-003` | Verify that: If job control is enabled, the AND-OR lists **must** form all or part of a foreground job that can be controlled as described in 2.11 Job Control. |

### 2. Shell Command Language > Shell Commands > Lists > AND Lists

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-09-03-06-001` | Verify that: The control operator "&&" denotes an AND list. The format **must** be: |
| `SHALL-19-09-03-06-002` | Verify that: First command1 **must** be executed. If its exit status is zero, command2 **must** be executed, and so on, until a command has a non-zero exit status or there are no more commands left to execute. The commands are expanded only if they are executed. |

### 2. Shell Command Language > Shell Commands > Lists > OR Lists

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-09-03-08-001` | Verify that: The control operator "\|\|" denotes an OR List. The format **must** be: |
| `SHALL-19-09-03-08-002` | Verify that: First, command1 **must** be executed. If its exit status is non-zero, command2 **must** be executed, and so on, until a command has a zero exit status or there are no more commands left to execute. |

### 2. Shell Command Language > Shell Commands > Compound Commands

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-09-04-001` | Verify that: The shell has several programming constructs that are "compound commands", which provide control flow for commands. Each of these compound commands has a reserved word or control operator at the beginning, and a corresponding terminator reserved word or operator at the end. In addition, each can be followed by redirections on the same line as the terminator. Each redirection **must** apply to all the commands within the compound command that do not explicitly override that redirection. |
| `SHALL-19-09-04-002` | Verify that: In the descriptions below, the exit status of some compound commands is stated in terms of the exit status of a compound-list. The exit status of a compound-list **must** be the value that the special parameter '?' (see 2.5.2 Special Parameters) would have immediately after execution of the compound-list. |

### 2. Shell Command Language > Shell Commands > Compound Commands > Grouping Commands

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-09-04-01-001` | Verify that: If a character sequence beginning with "((" would be parsed by the shell as an arithmetic expansion if preceded by a '$', shells which implement an extension whereby "((expression))" is evaluated as an arithmetic expression may treat the "((" as introducing as an arithmetic evaluation instead of a grouping command. A conforming application **must** ensure that it separates the two leading '(' characters with white space to prevent the shell from performing an arithmetic evaluation. |

### 2. Shell Command Language > Shell Commands > Compound Commands > Exit Status

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-09-04-02-001` | Verify that: The exit status of a grouping command **must** be the exit status of compound-list. |
| `SHALL-19-09-04-04-001` | Verify that: If there is at least one item in the list of items, the exit status of a for command **must** be the exit status of the last compound-list executed. If there are no items, the exit status **must** be zero. |
| `SHALL-19-09-04-06-001` | Verify that: The exit status of case **must** be zero if no patterns are matched. Otherwise, the exit status **must** be the exit status of the compound-list of the last clause to be executed. |
| `SHALL-19-09-04-08-001` | Verify that: The exit status of the if command **must** be the exit status of the then or else compound-list that was executed, or zero, if none was executed. |
| `SHALL-19-09-04-10-001` | Verify that: The exit status of the while loop **must** be the exit status of the last compound-list-2 executed, or zero if none was executed. |
| `SHALL-19-09-04-12-001` | Verify that: The exit status of the until loop **must** be the exit status of the last compound-list-2 executed, or zero if none was executed. |

### 2. Shell Command Language > Shell Commands > Compound Commands > The for Loop

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-09-04-03-001` | Verify that: The for loop **must** execute a sequence of commands for each member in a list of items. The for loop requires that the reserved words do and done be used to delimit the sequence of commands. |
| `SHALL-19-09-04-03-002` | Verify that: First, the list of words following in **must** be expanded to generate a list of items. Then, the variable name **must** be set to each item, in turn, and the compound-list executed each time. If no items result from the expansion, the compound-list **must** not be executed. Omitting: |
| `SHALL-19-09-04-03-003` | Verify that: **must** be equivalent to: |

### 2. Shell Command Language > Shell Commands > Compound Commands > Case Conditional Construct

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-09-04-05-001` | Verify that: The conditional construct case **must** execute the compound-list corresponding to the first pattern (see 2.14 Pattern Matching Notation), if any are present, that is matched by the string resulting from the tilde expansion, parameter expansion, command substitution, arithmetic expansion, and quote removal of the given word. The reserved word in **must** denote the beginning of the patterns to be matched. Multiple patterns with the same compound-list **must** be delimited by the '\|' symbol. The control operator ')' terminates a list of patterns corresponding to a given action. The terminated pattern list and the following compound-list is called a case statement clause. Each case statement clause, with the possible exception of the last, **must** be terminated with either ";;" or ";&". The case construct terminates with the reserved word esac (case reversed). |
| `SHALL-19-09-04-05-002` | Verify that: In order from the beginning to the end of the case statement, each pattern that labels a compound-list **must** be subjected to tilde expansion, parameter expansion, command substitution, and arithmetic expansion, and the result of these expansions **must** be compared against the expansion of word, according to the rules described in 2.14 Pattern Matching Notation (which also describes the effect of quoting parts of the pattern). After the first match, no more patterns in the case statement **must** be expanded, and the compound-list of the matching clause **must** be executed. If the case statement clause is terminated by ";;", no further clauses **must** be examined. If the case statement clause is terminated by ";&", then the compound-list (if any) of each subsequent clause **must** be executed, in order, until either a clause terminated by ";;" is reached and its compound-list (if any) executed or there are no further clauses in the case statement. The order of expansion and comparison of multiple patterns that label a compound-list statement is unspecified. |

### 2. Shell Command Language > Shell Commands > Compound Commands > The if Conditional Construct

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-09-04-07-001` | Verify that: The if command **must** execute a compound-list and use its exit status to determine whether to execute another compound-list. |
| `SHALL-19-09-04-07-002` | Verify that: The if compound-list **must** be executed; if its exit status is zero, the then compound-list **must** be executed and the command **must** complete. Otherwise, each elif compound-list **must** be executed, in turn, and if its exit status is zero, the then compound-list **must** be executed and the command **must** complete. Otherwise, the else compound-list **must** be executed. |

### 2. Shell Command Language > Shell Commands > Compound Commands > The while Loop

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-09-04-09-001` | Verify that: The while loop **must** continuously execute one compound-list as long as another compound-list has a zero exit status. |
| `SHALL-19-09-04-09-002` | Verify that: The compound-list-1 **must** be executed, and if it has a non-zero exit status, the while command **must** complete. Otherwise, the compound-list-2 **must** be executed, and the process **must** repeat. |

### 2. Shell Command Language > Shell Commands > Compound Commands > The until Loop

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-09-04-11-001` | Verify that: The until loop **must** continuously execute one compound-list as long as another compound-list has a non-zero exit status. |
| `SHALL-19-09-04-11-002` | Verify that: The compound-list-1 **must** be executed, and if it has a zero exit status, the until command completes. Otherwise, the compound-list-2 **must** be executed, and the process repeats. |

### 2. Shell Command Language > Shell Commands > Function Definition Command

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-09-05-001` | Verify that: The function is named fname; the application **must** ensure that it is a name (see XBD 3.216 Name) and that it is not the name of a special built-in utility. An implementation may allow other characters in a function name as an extension. The implementation **must** maintain separate name spaces for functions and variables. |
| `SHALL-19-09-05-002` | Verify that: When the function is declared, none of the expansions in 2.6 Word Expansions **must** be performed on the text in compound-command or io-redirect; all expansions **must** be performed as normal each time the function is called. Similarly, the optional io-redirect redirections and any variable assignments within compound-command **must** be performed during the execution of the function itself, not the function definition. See 2.8.1 Consequences of Shell Errors for the consequences of failures of these operations on interactive and non-interactive shells. |
| `SHALL-19-09-05-003` | Verify that: When a function is executed, it **must** have the syntax-error properties described for special built-in utilities in the first item in the enumerated list at the beginning of 2.15 Special Built-In Utilities. |
| `SHALL-19-09-05-004` | Verify that: The compound-command **must** be executed whenever the function name is specified as the name of a simple command (see 2.9.1.4 Command Search and Execution). The operands to the command temporarily **must** become the positional parameters during the execution of the compound-command; the special parameter '#' also **must** be changed to reflect the number of operands. The special parameter 0 **must** be unchanged. When the function completes, the values of the positional parameters and the special parameter '#' **must** be restored to the values they had before the function was executed. If the special built-in return (see return) is executed in the compound-command, the function completes and execution **must** resume with the next command after the function call. |

### 2. Shell Command Language > Shell Commands > Function Definition Command > Exit Status

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-09-05-01-001` | Verify that: The exit status of a function definition **must** be zero if the function was declared successfully; otherwise, it **must** be greater than zero. The exit status of a function invocation **must** be the exit status of the last command executed by the function. |

### 2. Shell Command Language > Shell Grammar

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-10-001` | Verify that: The following grammar defines the Shell Command Language. This formal syntax **must** take precedence over the preceding text syntax description. |

### 2. Shell Command Language > Shell Grammar > Shell Grammar Lexical Conventions

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-10-01-001` | Verify that: The input language to the shell **must** be first recognized at the character level. The resulting tokens **must** be classified by their immediate context according to the following rules (applied in order). These rules **must** be used to determine what a "token" is that is subject to parsing at the token level. The rules for token recognition in 2.3 Token Recognition **must** apply. |
| `SHALL-19-10-01-002` | Verify that: If the token is an operator, the token identifier for that operator **must** result. |
| `SHALL-19-10-01-003` | Verify that: If the string consists solely of digits and the delimiter character is one of '<' or '>', the token identifier IO_NUMBER **must** result. |
| `SHALL-19-10-01-004` | Verify that: If the string contains at least three characters, begins with a <left-curly-bracket> ('{') and ends with a <right-curly-bracket> ('}'), and the delimiter character is one of '<' or '>', the token identifier IO_LOCATION may result; if the result is not IO_LOCATION, the token identifier TOKEN **must** result. |
| `SHALL-19-10-01-005` | Verify that: Otherwise, the token identifier TOKEN **must** result. |
| `SHALL-19-10-01-006` | Verify that: Further distinction on TOKEN is context-dependent. It may be that the same TOKEN yields WORD, a NAME, an ASSIGNMENT_WORD, or one of the reserved words below, dependent upon the context. Some of the productions in the grammar below are annotated with a rule number from the following list. When a TOKEN is seen where one of those annotated productions could be used to reduce the symbol, the applicable rule **must** be applied to convert the token identifier type of the TOKEN to: |
| `SHALL-19-10-01-007` | Verify that: The reduction **must** then proceed based upon the token identifier type yielded by the rule applied. When more than one rule applies, the highest numbered rule **must** apply (which in turn may refer to another rule). (Note that except in rule 7, the presence of an '=' in the token has no effect.) |
| `SHALL-19-10-01-008` | Verify that: The WORD tokens **must** have the word expansion rules applied to them immediately before the associated command is executed, not at the time the command is parsed. |

### 2. Shell Command Language > Shell Grammar > Shell Grammar Rules

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-10-02-001` | Verify that: When the TOKEN is exactly a reserved word, the token identifier for that reserved word **must** result. Otherwise, the token WORD **must** be returned. Also, if the parser is in any state where only a reserved word could be the next correct token, proceed as above. |
| `SHALL-19-10-02-002` | Verify that: The expansions specified in 2.7 Redirection **must** occur. As specified there, exactly one field can result (or the result is unspecified), and there are additional requirements on pathname expansion. |
| `SHALL-19-10-02-003` | Verify that: Quote removal **must** be applied to the word to determine the delimiter that is used to find the end of the here-document that begins after the next <newline>. |
| `SHALL-19-10-02-004` | Verify that: When the TOKEN is exactly the reserved word esac, the token identifier for esac **must** result. Otherwise, the token WORD **must** be returned. |
| `SHALL-19-10-02-005` | Verify that: When the TOKEN meets the requirements for a name (see XBD 3.216 Name), the token identifier NAME **must** result. Otherwise, the token WORD **must** be returned. |
| `SHALL-19-10-02-006` | Verify that: When the TOKEN is exactly the reserved word in, the token identifier for in **must** result. Otherwise, the token WORD **must** be returned. |
| `SHALL-19-10-02-007` | Verify that: When the TOKEN is exactly the reserved word in or do, the token identifier for in or do **must** result, respectively. Otherwise, the token WORD **must** be returned. |
| `SHALL-19-10-02-008` | Verify that: If the TOKEN is exactly a reserved word, the token identifier for that reserved word **must** result. Otherwise, 7b **must** be applied. |
| `SHALL-19-10-02-009` | Verify that: If the TOKEN begins with '=', then the token WORD **must** be returned. |
| `SHALL-19-10-02-010` | Verify that: If all the characters in the TOKEN preceding the first such <equals-sign> form a valid name (see XBD 3.216 Name), the token ASSIGNMENT_WORD **must** be returned. |
| `SHALL-19-10-02-011` | Verify that: Otherwise, the token WORD **must** be returned. |
| `SHALL-19-10-02-012` | Verify that: If a returned ASSIGNMENT_WORD token begins with a valid name, assignment of the value after the first <equals-sign> to the name **must** occur as specified in 2.9.1 Simple Commands. If a returned ASSIGNMENT_WORD token does not begin with a valid name, the way in which the token is processed is unspecified. |
| `SHALL-19-10-02-013` | Verify that: When the TOKEN is exactly a reserved word, the token identifier for that reserved word **must** result. Otherwise, when the TOKEN meets the requirements for a name, the token identifier NAME **must** result. Otherwise, rule 7 applies. |
| `SHALL-19-10-02-014` | Verify that: Word expansion and assignment **must** never occur, even when required by the rules above, when this rule is being parsed. Each TOKEN that might either be expanded or have assignment applied to it **must** instead be returned as a single WORD consisting only of characters that are exactly the token described in 2.3 Token Recognition . |

### 2. Shell Command Language > Job Control

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-11-001` | Verify that: If the shell has a controlling terminal and it is the controlling process for the terminal session, it **must** initially set the foreground process group ID associated with the terminal to its own process group ID. Otherwise, if it has a controlling terminal, it **must** initially perform the following steps if interactive and may perform them if non-interactive: |
| `SHALL-19-11-002` | Verify that: If its process group is the foreground process group associated with the terminal, the shell **must** set its process group ID to its process ID (if they are not already equal) and set the foreground process group ID associated with the terminal to its process group ID. |
| `SHALL-19-11-003` | Verify that: If its process group is not the foreground process group associated with the terminal (which would result from it being started by a job-control shell as a background job), the shell **must** either stop itself by sending itself a SIGTTIN signal or, if interactive, attempt to read from standard input (which generates a SIGTTIN signal if standard input is the controlling terminal). If it is stopped, then when it continues execution (after receiving a SIGCONT signal) it **must** repeat these steps. |
| `SHALL-19-11-004` | Verify that: Subsequently, the shell **must** change the foreground process group associated with its controlling terminal when a foreground job is running as noted in the description below. |
| `SHALL-19-11-005` | Verify that: When job control is enabled, the shell **must** create one or more jobs when it executes a list (see 2.9.3 Lists) that has one of the following forms: |
| `SHALL-19-11-006` | Verify that: For the purposes of job control, a list that includes more than one asynchronous AND-OR list **must** be treated as if it were split into multiple separate lists, each ending with an asynchronous AND-OR list. |
| `SHALL-19-11-007` | Verify that: When a job consisting of a single asynchronous AND-OR list is created, it **must** form a background job and the associated process ID **must** be that of a child process that is made a process group leader, with all other processes (if any) that the shell creates to execute the AND-OR list initially having this process ID as their process group ID. |
| `SHALL-19-11-008` | Verify that: For a list consisting of one or more sequentially executed AND-OR lists followed by at most one asynchronous AND-OR list, the whole list **must** form a single foreground job up until the sequentially executed AND-OR lists have all completed execution, at which point the asynchronous AND-OR list (if any) **must** form a background job as described above. |
| `SHALL-19-11-009` | Verify that: For each pipeline in a foreground job, if the pipeline is executed while the list is still a foreground job, the set of processes comprising the pipeline, and any processes descended from it, **must** all be in the same process group, unless the shell executes some of the commands in the pipeline in the current shell execution environment and others in a subshell environment; in this case the process group ID of the current shell need not change (or cannot change if it is the session leader), and consequently the process group ID that the other processes all share may differ from the process group ID of the current shell (which means that a SIGSTOP, SIGTSTP, SIGTTIN, or SIGTTOU signal sent to one of those process groups does not cause the whole pipeline to stop). |
| `SHALL-19-11-010` | Verify that: A background job that was created on execution of an asynchronous AND-OR list can be brought into the foreground by means of the fg utility (if supported); in this case the entire job **must** become a single foreground job. If a process that the shell subsequently waits for is part of this foreground job and is stopped by a signal, the entire job **must** become a suspended job and the behavior **must** be as if the process had been stopped while the job was running in the background. |
| `SHALL-19-11-011` | Verify that: When a foreground job is created, or a background job is brought into the foreground by the fg utility, if the shell has a controlling terminal it **must** set the foreground process group ID associated with the terminal as follows: |
| `SHALL-19-11-012` | Verify that: If the job was originally created as a background job, the foreground process group ID **must** be set to the process ID of the process that the shell made a process group leader when it executed the asynchronous AND-OR list. |
| `SHALL-19-11-013` | Verify that: If the shell is not itself executing, in the current shell execution environment, all of the commands in the pipeline, the foreground process group ID **must** be set to the process group ID that is shared by the other processes executing the pipeline (see above). |
| `SHALL-19-11-014` | Verify that: If all of the commands in the pipeline are being executed by the shell itself in the current shell execution environment, the foreground process group ID **must** be set to the process group ID of the shell. |
| `SHALL-19-11-015` | Verify that: When a foreground job terminates, or becomes a suspended job (see below), if the shell has a controlling terminal it **must** set the foreground process group ID associated with the terminal to the process group ID of the shell. |
| `SHALL-19-11-016` | Verify that: Each background job (whether suspended or not) **must** have associated with it a job number and a process ID that is known in the current shell execution environment. When a background job is brought into the foreground by means of the fg utility, the associated job number **must** be removed from the shell's background jobs list and the associated process ID **must** be removed from the list of process IDs known in the current shell execution environment. |
| `SHALL-19-11-017` | Verify that: If the currently executing AND-OR list within the list comprising the foreground job consists of a single pipeline in which all of the commands are simple commands, the shell **must** either create a suspended job consisting of at least that AND-OR list and the remaining (if any) AND-OR lists in the same list, or create a suspended job consisting of just that AND-OR list and discard the remaining (if any) AND-OR lists in the same list. |
| `SHALL-19-11-018` | Verify that: Otherwise, the shell **must** create a suspended job consisting of a set of commands, from within the list comprising the foreground job, that is unspecified except that the set **must** include at least the pipeline to which the stopped process belongs. Commands in the foreground job that have not already completed and are not included in the suspended job **must** be discarded. |
| `SHALL-19-11-019` | Verify that: If a process that the shell is waiting for is part of a foreground job that was started as a foreground job and is stopped by a SIGSTOP signal, the behavior **must** be as described above for a catchable signal unless the shell was executing a built-in utility in the current shell execution environment when the SIGSTOP was delivered, resulting in the shell itself being stopped by the signal, in which case if the shell subsequently receives a SIGCONT signal and has one or more child processes that remain stopped, the shell **must** create a suspended job as if only those child processes had been stopped. |
| `SHALL-19-11-020` | Verify that: When a suspended job is created as a result of a foreground job being stopped, it **must** be assigned a job number, and an interactive shell **must** write, and a non-interactive shell may write, a message to standard error, formatted as described by the jobs utility (without the -l option) for a suspended job. The message may indicate that the commands comprising the job include commands that have already completed; in this case the completed commands **must** not be repeated if execution of the job is subsequently continued. If the shell is interactive, it **must** save the terminal settings before changing them to the settings it needs to read further commands. |
| `SHALL-19-11-021` | Verify that: When a process associated with a background job is stopped by a SIGSTOP, SIGTSTP, SIGTTIN, or SIGTTOU signal, the shell **must** convert the (non-suspended) background job into a suspended job and an interactive shell **must** write a message to standard error, formatted as described by the jobs utility (without the -l option) for a suspended job, at the following time: |
| `SHALL-19-11-022` | Verify that: If set -b is enabled, the message **must** be written either immediately after the job became suspended or immediately prior to writing the next prompt for input. |
| `SHALL-19-11-023` | Verify that: If set -b is disabled, the message **must** be written immediately prior to writing the next prompt for input. |
| `SHALL-19-11-024` | Verify that: Execution of a suspended job can be continued as a foreground job by means of the fg utility (if supported), or as a (non-suspended) background job either by means of the bg utility (if supported) or by sending the stopped processes a SIGCONT signal. The fg and bg utilities **must** send a SIGCONT signal to the process group of the process(es) whose stopped wait status caused the shell to suspend the job. If the shell has a controlling terminal, the fg utility **must** send the SIGCONT signal after it has set the foreground process group ID associated with the terminal (see above). If the fg utility is used from an interactive shell to bring into the foreground a suspended job that was created from a foreground job, before it sends the SIGCONT signal the fg utility **must** restore the terminal settings to the ones that the shell saved when the job was suspended. |
| `SHALL-19-11-025` | Verify that: When a background job completes or is terminated by a signal, an interactive shell **must** write a message to standard error, formatted as described by the jobs utility (without the -l option) for a job that completed or was terminated by a signal, respectively, at the following time: |
| `SHALL-19-11-026` | Verify that: If set -b is enabled, the message **must** be written immediately after the job completes or is terminated. |
| `SHALL-19-11-027` | Verify that: If set -b is disabled, the message **must** be written immediately prior to writing the next prompt for input. |

### 2. Shell Command Language > Signals and Error Handling

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-12-001` | Verify that: If job control is disabled (see the description of set -m) when the shell executes an asynchronous AND-OR list, the commands in the list **must** inherit from the shell a signal action of ignored (SIG_IGN) for the SIGINT and SIGQUIT signals. In all other cases, commands executed by the shell **must** inherit the same signal actions as those inherited by the shell from its parent unless a signal action is modified by the trap special built-in (see trap) |
| `SHALL-19-12-002` | Verify that: When a signal for which a trap has been set is received while the shell is waiting for the completion of a utility executing a foreground command, the trap associated with that signal **must** not be executed until after the foreground command has completed. When the shell is waiting, by means of the wait utility, for asynchronous commands to complete, the reception of a signal for which a trap has been set **must** cause the wait utility to return immediately with an exit status >128, immediately after which the trap associated with that signal **must** be taken. |

### 2. Shell Command Language > Shell Execution Environment

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-13-001` | Verify that: Utilities other than the special built-ins (see 2.15 Special Built-In Utilities) **must** be invoked in a separate environment that consists of the following. The initial value of these objects **must** be the same as that for the parent shell, except as noted below. |
| `SHALL-19-13-002` | Verify that: If the utility is a shell script, traps caught by the shell **must** be set to the default values and traps ignored by the shell **must** be set to be ignored by the utility; if the utility is not a shell script, the trap actions (default or ignore) **must** be mapped into the appropriate signal handling actions for the utility |
| `SHALL-19-13-003` | Verify that: Variables with the export attribute, along with those explicitly exported for the duration of the command, **must** be passed to the utility environment variables |
| `SHALL-19-13-004` | Verify that: The environment of the shell process **must** not be changed by the utility unless explicitly specified by the utility description (for example, cd and umask). |
| `SHALL-19-13-005` | Verify that: A subshell environment **must** be created as a duplicate of the shell environment, except that: |
| `SHALL-19-13-006` | Verify that: Unless specified otherwise (see trap), traps that are not being ignored **must** be set to the default action. |
| `SHALL-19-13-007` | Verify that: Changes made to the subshell environment **must** not affect the shell environment. Command substitution, commands that are grouped with parentheses, and asynchronous AND-OR lists **must** be executed in a subshell environment. Additionally, each command of a multi-command pipeline is in a subshell environment; as an extension, however, any or all commands in a pipeline may be executed in the current environment. Except where otherwise stated, all other commands **must** be executed in the current shell environment. |

### 2. Shell Command Language > Pattern Matching Notation > Patterns Matching a Single Character

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-14-01-001` | Verify that: The following patterns **must** match a single character: ordinary characters, special pattern characters, and pattern bracket expressions. The pattern bracket expression also **must** match a single collating element. |
| `SHALL-19-14-01-002` | Verify that: In a pattern, or part of one, where a shell-quoting <backslash> can be used, a <backslash> character **must** escape the following character as described in 2.2.1 Escape Character (Backslash), regardless of whether or not the <backslash> is inside a bracket expression. (The sequence "\\" represents one literal <backslash>.) |
| `SHALL-19-14-01-003` | Verify that: A <backslash> character that is not inside a bracket expression **must** preserve the literal value of the following character, unless the following character is in a part of the pattern where shell quoting can be used and is a shell quoting character, in which case the behavior is unspecified. |
| `SHALL-19-14-01-004` | Verify that: All of the requirements and effects of quoting on ordinary, shell special, and special pattern characters **must** apply to escaping in this context, except where specified otherwise. (Situations where this applies include word expansions when a pattern used in pathname expansion is not present in the original word but results from an earlier expansion, or the argument to the find -name or -path primary as passed to find, or the pattern argument to the fnmatch() and glob() functions when FNM_NOESCAPE or GLOB_NOESCAPE is not set in flags, respectively.) |
| `SHALL-19-14-01-005` | Verify that: An ordinary character is a pattern that **must** match itself. In a pattern, or part of one, where a shell-quoting <backslash> can be used, an ordinary character can be any character in the supported character set except for NUL, those special shell characters in 2.2 Quoting that require quoting, and the three special pattern characters described below. In a pattern, or part of one, where a shell-quoting <backslash> cannot be used to preserve the literal value of a character that would otherwise be treated as special, an ordinary character can be any character in the supported character set except for NUL and the three special pattern characters described below. Matching **must** be based on the bit pattern used for encoding the character, not on the graphic representation of the character. If any character (ordinary, shell special, or pattern special) is quoted, or escaped with a <backslash>, that pattern **must** match the character itself. The application **must** ensure that it quotes or escapes any character that would otherwise be treated as special, in order for it to be matched as an ordinary character. |
| `SHALL-19-14-01-006` | Verify that: When unquoted, unescaped, and not inside a bracket expression, the following three characters **must** have special meaning in the specification of patterns: |
| `SHALL-19-14-01-007` | Verify that: A <question-mark> is a pattern that **must** match any character. |
| `SHALL-19-14-01-008` | Verify that: An <asterisk> is a pattern that **must** match multiple characters, as described in 2.14.2 Patterns Matching Multiple Characters. |
| `SHALL-19-14-01-009` | Verify that: A <left-square-bracket> **must** introduce a bracket expression if the characters following it meet the requirements for bracket expressions stated in XBD 9.3.5 RE Bracket Expression, except that the <exclamation-mark> character ('!') **must** replace the <circumflex> character ('^') in its role in a non-matching list in the regular expression notation. A bracket expression starting with an unquoted <circumflex> character produces unspecified results. A <left-square-bracket> that does not introduce a valid bracket expression **must** match the character itself. |

### 2. Shell Command Language > Pattern Matching Notation > Patterns Matching Multiple Characters

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-14-02-001` | Verify that: The <asterisk> ('*') is a pattern that **must** match any string, including the null string. |
| `SHALL-19-14-02-002` | Verify that: The concatenation of patterns matching a single character is a valid pattern that **must** match the concatenation of the single characters or collating elements matched by each of the concatenated patterns. |
| `SHALL-19-14-02-003` | Verify that: The concatenation of one or more patterns matching a single character with one or more <asterisk> characters is a valid pattern. In such patterns, each <asterisk> **must** match a string of zero or more characters, matching the greatest possible number of characters that still allows the remainder of the pattern to match the string. |

### 2. Shell Command Language > Pattern Matching Notation > Patterns Used for Filename Expansion

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-14-03-001` | Verify that: The <slash> character in a pathname **must** be explicitly matched by using one or more <slash> characters in the pattern; it **must** neither be matched by the <asterisk> or <question-mark> special characters nor by a bracket expression. <slash> characters in the pattern **must** be identified before bracket expressions; thus, a <slash> cannot be included in a pattern bracket expression used for filename expansion. If a <slash> character is found following an unescaped <left-square-bracket> character before a corresponding <right-square-bracket> is found, the open bracket **must** be treated as an ordinary character. For example, the pattern "a[b/c]d" does not match such pathnames as abd or a/d. It only matches a pathname of literally a[b/c]d. |
| `SHALL-19-14-03-002` | Verify that: If the pattern matches any existing filenames or pathnames, the pattern **must** be replaced with those filenames and pathnames, sorted according to the collating sequence in effect in the current locale. If this collating sequence does not have a total ordering of all characters (see XBD 7.3.2 LC_COLLATE), any filenames or pathnames that collate equally **must** be further compared byte-by-byte using the collating sequence for the POSIX locale. |
| `SHALL-19-14-03-003` | Verify that: If the pattern does not match any existing filenames or pathnames, the pattern string **must** be left unchanged. |
| `SHALL-19-14-03-004` | Verify that: If a specified pattern does not contain any '*', '?' or '[' characters that will be treated as special, the pattern string **must** be left unchanged. |

### 2. Shell Command Language > Special Built-In Utilities

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-15-001` | Verify that: The following "special built-in" utilities **must** be supported in the shell command language. The output of each command, if any, **must** be written to standard output, subject to the normal redirection and piping possible with all commands. |
| `SHALL-19-15-002` | Verify that: An error in a special built-in utility may cause a shell executing that utility to abort, while an error in a regular built-in utility **must** not cause a shell executing that utility to abort. (See 2.8.1 Consequences of Shell Errors for the consequences of errors on interactive and non-interactive shells.) If a special built-in utility encountering an error does not abort the shell, its exit value **must** be non-zero. |
| `SHALL-19-15-003` | Verify that: As described in 2.9.1 Simple Commands, variable assignments preceding the invocation of a special built-in utility affect the current execution environment; this **must** not be the case with a regular built-in or other utility. |
| `SHALL-19-15-004` | Verify that: Some of the special built-ins are described as conforming to XBD 12.2 Utility Syntax Guidelines. For those that are not, the requirement in 1.4 Utility Description Defaults that "--" be recognized as a first argument to be discarded does not apply and a conforming application **must** not use that argument. |

### 2. Shell Command Language > Special Built-In Utilities > DESCRIPTION

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-16-03-001` | Verify that: If n is specified, the break utility **must** exit from the nth enclosing for, while, or until loop. If n is not specified, break **must** behave as if n was specified as 1. Execution **must** continue with the command immediately following the exited loop. The application **must** ensure that the value of n is a positive decimal integer. If n is greater than the number of enclosing loops, the outermost enclosing loop **must** be exited. If there is no enclosing loop, the behavior is unspecified. |
| `SHALL-19-16-03-002` | Verify that: A loop **must** enclose a break or continue command if the loop lexically encloses the command. A loop lexically encloses a break or continue command if the command is: |
| `SHALL-19-17-03-001` | Verify that: This utility **must** do nothing except return a 0 exit status. It is used when a command is needed, as in the then condition of an if command, but nothing is to be done by the command. |
| `SHALL-19-18-03-001` | Verify that: If n is specified, the continue utility **must** return to the top of the nth enclosing for, while, or until loop. If n is not specified, continue **must** behave as if n was specified as 1. Returning to the top of the loop involves repeating the condition list of a while or until loop or performing the next assignment of a for loop, and re-executing the loop if appropriate. |
| `SHALL-19-18-03-002` | Verify that: The application **must** ensure that the value of n is a positive decimal integer. If n is greater than the number of enclosing loops, the outermost enclosing loop **must** be used. If there is no enclosing loop, the behavior is unspecified. |
| `SHALL-19-18-03-003` | Verify that: The meaning of "enclosing" **must** be as specified in the description of the break utility. |
| `SHALL-19-19-03-001` | Verify that: The shell **must** tokenize (see 2.3 Token Recognition) the contents of the file, parse the tokens (see 2.10 Shell Grammar), and execute the resulting commands in the current environment. It is unspecified whether the commands are parsed and executed as a program (as for a shell script) or are parsed as a single compound_list that is executed after the entire file has been parsed. |
| `SHALL-19-19-03-002` | Verify that: If file does not contain a <slash>, the shell **must** use the search path specified by PATH to find the directory containing file. Unlike normal command search, however, the file searched for by the dot utility need not be executable. If no readable file is found, a non-interactive shell **must** abort; an interactive shell **must** write a diagnostic message to standard error. |
| `SHALL-19-19-03-003` | Verify that: The dot special built-in **must** support XBD 12.2 Utility Syntax Guidelines, except for Guidelines 1 and 2. |
| `SHALL-19-20-03-001` | Verify that: The eval utility **must** construct a command string by concatenating arguments together, separating each with a <space> character. The constructed command string **must** be tokenized (see 2.3 Token Recognition), parsed (see 2.10 Shell Grammar), and executed by the shell in the current environment. It is unspecified whether the commands are parsed and executed as a program (as for a shell script) or are parsed as a single compound_list that is executed after the entire constructed command string has been parsed. |
| `SHALL-19-21-03-001` | Verify that: If exec is specified with no operands, any redirections associated with the exec command **must** be made in the current shell execution environment. If any file descriptors with numbers greater than 2 are opened by those redirections, it is unspecified whether those file descriptors remain open when the shell invokes another utility. Scripts concerned that child shells could misuse open file descriptors can always close them explicitly, as shown in one of the following examples. If the result of the redirections would be that file descriptor 0, 1, or 2 is closed, implementations may open the file descriptor to an unspecified file. |
| `SHALL-19-21-03-002` | Verify that: If exec is specified with a utility operand, the shell **must** execute a non-built-in utility as described in 2.9.1.6 Non-built-in Utility Execution with utility as the command name and the argument operands (if any) as the command arguments. |
| `SHALL-19-21-03-003` | Verify that: If the exec command fails, a non-interactive shell **must** exit from the current shell execution environment; [UP] an interactive shell may exit from a subshell environment but **must** not exit if the current shell environment is not a subshell environment. |
| `SHALL-19-21-03-004` | Verify that: If the exec command fails and the shell does not exit, any redirections associated with the exec command that were successfully made **must** take effect in the current shell execution environment. |
| `SHALL-19-21-03-005` | Verify that: The exec special built-in **must** support XBD 12.2 Utility Syntax Guidelines. |
| `SHALL-19-22-03-001` | Verify that: The exit utility **must** cause the shell to exit from its current execution environment. If the current execution environment is a subshell environment, the shell **must** exit from the subshell environment and continue in the environment from which that subshell environment was invoked; otherwise, the shell utility **must** terminate. The wait status of the shell or subshell **must** be determined by the unsigned decimal integer n, if specified. |
| `SHALL-19-22-03-002` | Verify that: If n is specified and has a value between 0 and 255 inclusive, the wait status of the shell or subshell **must** indicate that it exited with exit status n. If n is specified and has a value greater than 256 that corresponds to an exit status the shell assigns to commands terminated by a valid signal (see 2.8.2 Exit Status for Commands), the wait status of the shell or subshell **must** indicate that it was terminated by that signal. No other actions associated with the signal, such as execution of trap actions or creation of a core image, **must** be performed by the shell. |
| `SHALL-19-22-03-003` | Verify that: If n is not specified, the result **must** be as if n were specified with the current value of the special parameter '?' (see 2.5.2 Special Parameters), except that if the exit command would cause the end of execution of a trap action, the value for the special parameter '?' that is considered "current" **must** be the value it had immediately preceding the trap action. |
| `SHALL-19-22-03-004` | Verify that: A trap action on EXIT **must** be executed before the shell terminates, except when the exit utility is invoked in that trap action itself, in which case the shell **must** exit immediately. It is unspecified whether setting a new trap action on EXIT during execution of a trap action on EXIT will cause the new trap action to be executed before the shell terminates. |
| `SHALL-19-23-03-001` | Verify that: The shell **must** give the export attribute to the variables corresponding to the specified names, which **must** cause them to be in the environment of subsequently executed commands. If the name of a variable is followed by =word, then the value of that variable **must** be set to word. |
| `SHALL-19-23-03-002` | Verify that: The export special built-in **must** be a declaration utility. Therefore, if export is recognized as the command name of a simple command, then subsequent words of the form name=word **must** be expanded in an assignment context. See 2.9.1.1 Order of Processing. |
| `SHALL-19-23-03-003` | Verify that: The export special built-in **must** support XBD 12.2 Utility Syntax Guidelines. |
| `SHALL-19-23-03-004` | Verify that: The shell **must** format the output, including the proper use of quoting, so that it is suitable for reinput to the shell as commands that achieve the same exporting results, except: |
| `SHALL-19-24-03-001` | Verify that: The variables whose names are specified **must** be given the readonly attribute. The values of variables with the readonly attribute cannot be changed by subsequent assignment or use of the export, getopts, readonly, or read utilities, nor can those variables be unset by the unset utility. As described in XBD 8.1 Environment Variable Definition, conforming applications **must** not request to mark a variable as readonly if it is documented as being manipulated by a shell built-in utility, as it may render those utilities unable to complete successfully. If the name of a variable is followed by =word, then the value of that variable **must** be set to word. |
| `SHALL-19-24-03-002` | Verify that: The readonly special built-in **must** be a declaration utility. Therefore, if readonly is recognized as the command name of a simple command, then subsequent words of the form name=word **must** be expanded in an assignment context. See 2.9.1.1 Order of Processing. |
| `SHALL-19-24-03-003` | Verify that: The readonly special built-in **must** support XBD 12.2 Utility Syntax Guidelines. |
| `SHALL-19-24-03-004` | Verify that: The shell **must** format the output, including the proper use of quoting, so that it is suitable for reinput to the shell as commands that achieve the same value and readonly attribute-setting results in a shell execution environment in which: |
| `SHALL-19-25-03-001` | Verify that: The return utility **must** cause the shell to stop executing the current function or dot script. If the shell is not currently executing a function or dot script, the results are unspecified. |
| `SHALL-19-26-03-001` | Verify that: If no options or arguments are specified, set **must** write the names and values of all shell variables in the collation sequence of the current locale. Each name **must** start on a separate line, using the format: |
| `SHALL-19-26-03-002` | Verify that: The value string **must** be written with appropriate quoting; see the description of shell quoting in 2.2 Quoting. The output **must** be suitable for reinput to the shell, setting or resetting, as far as possible, the variables that are currently set; read-only variables cannot be reset. |
| `SHALL-19-26-03-003` | Verify that: When options are specified, they **must** set or unset attributes of the shell, as described below. When arguments are specified, they cause positional parameters to be set or unset, as described below. Setting or unsetting attributes and positional parameters are not necessarily related actions, but they can be combined in a single invocation of set. |
| `SHALL-19-26-03-004` | Verify that: The set special built-in **must** support XBD 12.2 Utility Syntax Guidelines except that options can be specified with either a leading <hyphen-minus> (meaning enable the option) or <plus-sign> (meaning disable it) unless otherwise specified. |
| `SHALL-19-26-03-005` | Verify that: Implementations **must** support the options in the following list in both their <hyphen-minus> and <plus-sign> forms. These options can also be specified as options to sh. |
| `SHALL-19-26-03-006` | Verify that: This option **must** be supported if the implementation supports the User Portability Utilities option. When job control and -b are both enabled, the shell **must** write asynchronous notifications of background job completions (including termination by a signal), and may write asynchronous notifications of background job suspensions. See 2.11 Job Control for details. When job control is disabled, the -b option **must** have no effect. Asynchronous notification **must** not be enabled by default. |
| `SHALL-19-26-03-007` | Verify that: (Uppercase C.) Prevent existing regular files from being overwritten by the shell's '>' redirection operator (see 2.7.2 Redirecting Output); the ">\|" redirection operator **must** override this noclobber option for an individual file. |
| `SHALL-19-26-03-008` | Verify that: The failure of any individual command in a multi-command pipeline, or of any subshell environments in which command substitution was performed during word expansion, **must** not cause the shell to exit. Only the failure of the pipeline itself **must** be considered. |
| `SHALL-19-26-03-009` | Verify that: The -e setting **must** be ignored when executing the compound list following the while, until, if, or elif reserved word, a pipeline beginning with the ! reserved word, or any command of an AND-OR list other than the last. |
| `SHALL-19-26-03-010` | Verify that: If the exit status of a compound command other than a subshell command was the result of a failure while -e was being ignored, then -e **must** not apply to this command. |
| `SHALL-19-26-03-011` | Verify that: The shell **must** disable pathname expansion. |
| `SHALL-19-26-03-012` | Verify that: This option **must** be supported if the implementation supports the User Portability Utilities option. When this option is enabled, the shell **must** perform job control actions as described in 2.11 Job Control. This option **must** be enabled by default for interactive shells. |
| `SHALL-19-26-03-013` | Verify that: The shell **must** read commands but does not execute them; this can be used to check for shell script syntax errors. Interactive shells and subshells of interactive shells, recursively, may ignore this option. |
| `SHALL-19-26-03-014` | Verify that: Prevent an interactive shell from exiting on end-of-file. This setting prevents accidental logouts when <control>-D is entered. A user **must** explicitly exit to leave the interactive shell. This option **must** be supported if the system supports the User Portability Utilities option. |
| `SHALL-19-26-03-015` | Verify that: Equivalent to -m. This option **must** be supported if the system supports the User Portability Utilities option. |
| `SHALL-19-26-03-016` | Verify that: When the shell tries to expand, in a parameter expansion or an arithmetic expansion, an unset parameter other than the '@' and '*' special parameters, it **must** write a message to standard error and the expansion **must** fail with the consequences specified in 2.8.1 Consequences of Shell Errors. |
| `SHALL-19-26-03-017` | Verify that: The shell **must** write its input to standard error as it is read. |
| `SHALL-19-26-03-018` | Verify that: The shell **must** write to standard error a trace for each command after it expands the command and before it executes it. It is unspecified whether the command that turns tracing off is traced. |
| `SHALL-19-26-03-019` | Verify that: The default for all these options **must** be off (unset) unless stated otherwise in the description of the option or unless the shell was invoked with them on; see sh. |
| `SHALL-19-26-03-020` | Verify that: The remaining arguments **must** be assigned in order to the positional parameters. The special parameter '#' **must** be set to reflect the number of positional parameters. All positional parameters **must** be unset before any new values are assigned. |
| `SHALL-19-26-03-021` | Verify that: The special argument "--" immediately following the set command name can be used to delimit the arguments if the first argument begins with '+' or '-', or to prevent inadvertent listing of all shell variables when there are no arguments. The command set -- without argument **must** unset all positional parameters and set the special parameter '#' to zero. |
| `SHALL-19-27-03-001` | Verify that: The positional parameters **must** be shifted. Positional parameter 1 **must** be assigned the value of parameter (1+n), parameter 2 **must** be assigned the value of parameter (2+n), and so on. The parameters represented by the numbers "$#" down to "$#-n+1" **must** be unset, and the parameter '#' is updated to reflect the new number of positional parameters. |
| `SHALL-19-27-03-002` | Verify that: The value n **must** be an unsigned decimal integer less than or equal to the value of the special parameter '#'. If n is not given, it **must** be assumed to be 1. If n is 0, the positional and special parameters are not changed. |
| `SHALL-19-28-03-001` | Verify that: The times utility **must** write the accumulated user and system times for the shell and for all of its child processes, in the following POSIX locale format: |
| `SHALL-19-28-03-002` | Verify that: The four pairs of times **must** correspond to the members of the <sys/times.h> tms structure (defined in XBD 14. Headers) as returned by times(): tms_utime, tms_stime, tms_cutime, and tms_cstime, respectively. |
| `SHALL-19-29-03-001` | Verify that: If the -p option is not specified and the first operand is an unsigned decimal integer, the shell **must** treat all operands as conditions, and **must** reset each condition to the default value. Otherwise, if the -p option is not specified and there are operands, the first operand **must** be treated as an action and the remaining as conditions. |
| `SHALL-19-29-03-002` | Verify that: If action is '-', the shell **must** reset each condition to the default value. If action is null (""), the shell **must** ignore each specified condition if it arises. Otherwise, the argument action **must** be read and executed by the shell when one of the corresponding conditions arises. The action of trap **must** override a previous action (either default action or one explicitly set). The value of "$?" after the trap action completes **must** be the value it had before the trap action was executed. |
| `SHALL-19-29-03-003` | Verify that: The EXIT condition **must** occur when the shell terminates normally (exits), and may occur when the shell terminates abnormally as a result of delivery of a signal (other than SIGKILL) whose trap action is the default. |
| `SHALL-19-29-03-004` | Verify that: The environment in which the shell executes a trap action on EXIT **must** be identical to the environment immediately after the last command executed before the trap action on EXIT was executed. |
| `SHALL-19-29-03-005` | Verify that: If action is neither '-' nor the empty string, then each time a matching condition arises, the action **must** be executed in a manner equivalent to: |
| `SHALL-19-29-03-006` | Verify that: Signals that were ignored on entry to a non-interactive shell cannot be trapped or reset, although no error need be reported when attempting to do so. An interactive shell may reset or catch signals ignored on entry. Traps **must** remain in place for a given shell until explicitly changed with another trap command. |
| `SHALL-19-29-03-007` | Verify that: When a subshell is entered, traps that are not being ignored **must** be set to the default actions, except in the case of a command substitution containing only a single trap command, when the traps need not be altered. Implementations may check for this case using only lexical analysis; for example, if `trap` and $( trap -- ) do not alter the traps in the subshell, cases such as assigning var=trap and then using $($var) may still alter them. This does not imply that the trap command cannot be used within the subshell to set new traps. |
| `SHALL-19-29-03-008` | Verify that: The trap command with no operands **must** write to standard output a list of commands associated with each of a set of conditions; if the -p option is not specified, this set **must** contain only the conditions that are not in the default state (including signals that were ignored on entry to a non-interactive shell); if the -p option is specified, the set **must** contain all conditions, except that it is unspecified whether conditions corresponding to the SIGKILL and SIGSTOP signals are included in the set. If the command is executed in a subshell, the implementation does not perform the optional check described above for a command substitution containing only a single trap command, and no trap commands with operands have been executed since entry to the subshell, the list **must** contain the commands that were associated with each condition immediately before the subshell environment was entered. Otherwise, the list **must** contain the commands currently associated with each condition. The format **must** be: |
| `SHALL-19-29-03-009` | Verify that: The shell **must** format the output, including the proper use of quoting, so that it is suitable for reinput to the shell as commands that achieve the same trapping results for the set of conditions included in the output, except for signals that were ignored on entry to the shell as described above. If this set includes conditions corresponding to the SIGKILL and SIGSTOP signals, the shell **must** accept them when the output is reinput to the shell (where accepting them means they do not cause a non-zero exit status, a diagnostic message, or undefined behavior). For example: |
| `SHALL-19-29-03-010` | Verify that: If an invalid signal name [XSI] or number is specified, the trap utility **must** write a warning message to standard error. |
| `SHALL-19-29-03-011` | Verify that: The trap special built-in **must** conform to XBD 12.2 Utility Syntax Guidelines. |
| `SHALL-19-30-03-001` | Verify that: The unset utility **must** unset each variable or function definition specified by name that does not have the readonly attribute and remove any attributes other than readonly that have been given to name (see 2.15 Special Built-In Utilities export and readonly). |
| `SHALL-19-30-03-002` | Verify that: If -v is specified, name refers to a variable name and the shell **must** unset it and remove it from the environment. Read-only variables cannot be unset. |
| `SHALL-19-30-03-003` | Verify that: If -f is specified, name refers to a function and the shell **must** unset the function definition. |
| `SHALL-19-30-03-004` | Verify that: If neither -f nor -v is specified, name refers to a variable; if a variable by that name does not exist, it is unspecified whether a function by that name, if any, **must** be unset. |
| `SHALL-19-30-03-005` | Verify that: Unsetting a variable or function that was not previously set **must** not be considered an error and does not cause the shell to abort. |
| `SHALL-19-30-03-006` | Verify that: The unset special built-in **must** support XBD 12.2 Utility Syntax Guidelines. |

### 2. Shell Command Language > Special Built-In Utilities > OPTIONS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-17-04-001` | Verify that: This utility **must** not recognize the "--" argument in the manner specified by Guideline 10 of XBD 12.2 Utility Syntax Guidelines. |
| `SHALL-19-17-04-002` | Verify that: Implementations **must** not support any options. |
| `SHALL-19-29-04-001` | Verify that: The following option **must** be supported: |
| `SHALL-19-29-04-002` | Verify that: The shell **must** format the output, including the proper use of quoting, so that it is suitable for reinput to the shell as commands that achieve the same trapping results for the specified set of conditions. If a condition operand is a condition corresponding to the SIGKILL or SIGSTOP signal, and trap -p without any operands would not include it in the set of conditions for which it writes output, the behavior is undefined if the output is reinput to the shell. |

### 2. Shell Command Language > Special Built-In Utilities > EXIT STATUS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-19-14-001` | Verify that: If no readable file was found or if the commands in the file could not be parsed, and the shell is interactive (and therefore does not abort; see 2.8.1 Consequences of Shell Errors), the exit status **must** be non-zero. Otherwise, return the value of the last command executed, or a zero exit status if no command is executed. |
| `SHALL-19-20-14-001` | Verify that: If there are no arguments, or only null arguments, eval **must** return a zero exit status; otherwise, it **must** return the exit status of the command defined by the string of concatenated arguments separated by <space> characters, or a non-zero exit status if the concatenation could not be parsed as a command and the shell is interactive (and therefore did not abort). |
| `SHALL-19-21-14-001` | Verify that: If utility is specified and is executed, exec **must** not return to the shell; rather, the exit status of the current shell execution environment **must** be the exit status of utility. If utility is specified and an attempt to execute it as a non-built-in utility fails, the exit status **must** be as described in 2.9.1.6 Non-built-in Utility Execution. If a redirection error occurs (see 2.8.1 Consequences of Shell Errors), the exit status **must** be a value in the range 1-125. Otherwise, exec **must** return a zero exit status. |
| `SHALL-19-25-14-001` | Verify that: The exit status **must** be n, if specified, except that the behavior is unspecified if n is not an unsigned decimal integer or is greater than 255. If n is not specified, the result **must** be as if n were specified with the current value of the special parameter '?' (see 2.5.2 Special Parameters), except that if the return command would cause the end of execution of a trap action, the value for the special parameter '?' that is considered "current" **must** be the value it had immediately preceding the trap action. |
| `SHALL-19-27-14-001` | Verify that: If the n operand is invalid or is greater than "$#", this may be treated as an error and a non-interactive shell may exit; if the shell does not exit in this case, a non-zero exit status **must** be returned and a warning message **must** be written to standard error. Otherwise, zero **must** be returned. |
| `SHALL-19-29-14-001` | Verify that: If the trap name [XSI] or number is invalid, a non-zero exit status **must** be returned; otherwise, zero **must** be returned. For both interactive and non-interactive shells, invalid signal names [XSI] or numbers **must** not be considered an error and **must** not cause the shell to abort. |

### 2. Shell Command Language > Special Built-In Utilities > ENVIRONMENT VARIABLES

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-21-08-001` | Verify that: The following environment variable **must** affect the execution of exec: |

### 2. Shell Command Language > Special Built-In Utilities > Issue 6

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-23-22-001` | Verify that: IEEE Std 1003.1-2001/Cor 1-2002, item XCU/TC1/D6/6 is applied, adding the following text to the end of the first paragraph of the DESCRIPTION: "If the name of a variable is followed by =word, then the value of that variable **must** be set to word.". The reason for this change is that the SYNOPSIS for export includes: |
| `SHALL-19-24-22-001` | Verify that: IEEE Std 1003.1-2001/Cor 1-2002, item XCU/TC1/D6/7 is applied, adding the following text to the end of the first paragraph of the DESCRIPTION: "If the name of a variable is followed by =word, then the value of that variable **must** be set to word.". The reason for this change is that the SYNOPSIS for readonly includes: |
| `SHALL-19-28-22-001` | Verify that: IEEE Std 1003.1-2001/Cor 1-2002, item XCU/TC1/D6/9 is applied, changing text in the DESCRIPTION from: "Write the accumulated user and system times for the shell and for all of its child processes ..." to: "The times utility **must** write the accumulated user and system times for the shell and for all of its child processes ...". |

### 2. Shell Command Language > Special Built-In Utilities > RATIONALE

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-19-26-18-001` | Verify that: The ignoreeof setting prevents accidental logouts when the end-of-file character (typically <control>-D) is entered. A user **must** explicitly exit to leave the interactive shell. |

## sh Utility

### DESCRIPTION

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-110-03-001` | Verify that: Pathname expansion **must** not fail due to the size of a file. |

### OPTIONS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-110-04-001` | Verify that: The sh utility **must** conform to XBD 12.2 Utility Syntax Guidelines, with an extension for support of a leading <plus-sign> ('+') as noted below. |
| `SHALL-20-110-04-002` | Verify that: The following additional options **must** be supported: |
| `SHALL-20-110-04-003` | Verify that: If there are no operands and the -c option is not specified, the -s option **must** be assumed. |

### OPERANDS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-110-05-001` | Verify that: The following operands **must** be supported: |
| `SHALL-20-110-05-002` | Verify that: A single <hyphen-minus> **must** be treated as the first operand and then ignored. If both '-' and "--" are given as arguments, or if other operands precede the single <hyphen-minus>, the results are undefined. |
| `SHALL-20-110-05-003` | Verify that: The positional parameters ($1, $2, and so on) **must** be set to arguments, if any. |
| `SHALL-20-110-05-004` | Verify that: The implementation **must** attempt to read that file from the current working directory; the file need not be executable. |
| `SHALL-20-110-05-005` | Verify that: Special parameter 0 (see 2.5.2 Special Parameters) **must** be set to the value of command_file. If sh is called using a synopsis form that omits command_file, special parameter 0 **must** be set to the value of the first argument passed to sh from its parent (for example, argv[0] for a C program), which is normally a pathname used to execute the sh utility. |
| `SHALL-20-110-05-006` | Verify that: A string assigned to special parameter 0 when executing the commands in command_string. If command_name is not specified, special parameter 0 **must** be set to the value of the first argument passed to sh from its parent (for example, argv[0] for a C program), which is normally a pathname used to execute the sh utility. |
| `SHALL-20-110-05-007` | Verify that: A string that **must** be interpreted by the shell as one or more commands, as if the string were the argument to the system() function defined in the System Interfaces volume of POSIX.1-2024. If the command_string operand is an empty string, sh **must** exit with a zero exit status. |

### INPUT FILES

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-110-07-001` | Verify that: The input file can be of any type, but the initial portion of the file intended to be parsed according to the shell grammar (see 2.10.2 Shell Grammar Rules) **must** consist of characters and **must** not contain the NUL character. The shell **must** not enforce any line length limits. If the input file consists solely of zero or more blank lines and comments, sh **must** exit with a zero exit status. |

### ENVIRONMENT VARIABLES

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-110-08-001` | Verify that: The following environment variables **must** affect the execution of sh: |
| `SHALL-20-110-08-002` | Verify that: [UP] This variable, when and only when an interactive shell is invoked, **must** be subjected to parameter expansion (see 2.6.2 Parameter Expansion) by the shell, and the resulting value **must** be used as a pathname of a file containing shell commands to execute in the current environment. The file need not be executable. If the expanded value of ENV is not an absolute pathname, the results are unspecified. ENV **must** be ignored if the real and effective user IDs or real and effective group IDs of the process are different. The file specified by ENV need not be processed if the file can be written by any user other than the user identified by the real (and effective) user ID of the shell process. |
| `SHALL-20-110-08-003` | Verify that: [UP] This variable, when expanded by the shell, **must** determine the default value for the -e editor option's editor option-argument. If FCEDIT is null or unset, ed **must** be used as the editor. |
| `SHALL-20-110-08-004` | Verify that: [UP] Determine a decimal number representing the limit to the number of previous commands that are accessible. If this variable is unset, an unspecified default greater than or equal to 128 **must** be used. The maximum number of commands in the history list is unspecified, but **must** be at least 128. An implementation may choose to access this variable only when initializing the history file, as described under HISTFILE . Therefore, it is unspecified whether changes made to HISTSIZE after the history file has been initialized are effective. |
| `SHALL-20-110-08-005` | Verify that: [UP] Determine a pathname of the user's mailbox file for purposes of incoming mail notification. If this variable is set, the shell **must** inform the user if the file named by the variable is created or if its modification time has changed. Informing the user **must** be accomplished by writing a string of unspecified format to standard error prior to the writing of the next primary prompt string. Such check **must** be performed only after the completion of the interval defined by the MAILCHECK variable after the last such check. The user **must** be informed only if MAIL is set and MAILPATH is not set. |
| `SHALL-20-110-08-006` | Verify that: [UP] Establish a decimal integer value that specifies how often (in seconds) the shell **must** check for the arrival of mail in the files specified by the MAILPATH or MAIL variables. The default value **must** be 600 seconds. If set to zero, the shell **must** check before issuing each primary prompt. |
| `SHALL-20-110-08-007` | Verify that: This variable **must** represent an absolute pathname of the current working directory. Assignments to this variable may be ignored. |

### ASYNCHRONOUS EVENTS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-110-09-001` | Verify that: If the shell is interactive, SIGINT signals received during command line editing **must** be handled as described in the EXTENDED DESCRIPTION, and SIGINT signals received at other times **must** be caught but no action performed. |
| `SHALL-20-110-09-002` | Verify that: SIGQUIT and SIGTERM signals **must** be ignored. |
| `SHALL-20-110-09-003` | Verify that: If the -m option is in effect, SIGTTIN, SIGTTOU, and SIGTSTP signals **must** be ignored. |
| `SHALL-20-110-09-004` | Verify that: If the -m option is not in effect, it is unspecified whether SIGTTIN, SIGTTOU, and SIGTSTP signals are ignored, set to the default action, or caught. If they are caught, the shell **must**, in the signal-catching function, set the signal to the default action and raise the signal (after taking any appropriate steps, such as restoring terminal settings). |

### STDERR

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-110-11-001` | Verify that: Except as otherwise stated (by the descriptions of any invoked utilities or in interactive mode), standard error **must** be used only for diagnostic messages. |

### EXTENDED DESCRIPTION > Command History List

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-110-13-01-001` | Verify that: When the sh utility is being used interactively, it **must** maintain a list of commands previously entered from the terminal in the file named by the HISTFILE environment variable. The type, size, and internal format of this file are unspecified. Multiple sh processes can share access to the file for a user, if file access permissions allow this; see the description of the HISTFILE environment variable. |

### EXTENDED DESCRIPTION > Command Line Editing

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-110-13-02-001` | Verify that: The command set -o vi **must** enable vi-mode editing and place sh into vi insert mode (see Command Line Editing (vi-mode)). This command also **must** disable any other editing mode that the implementation may provide. The command set +o vi disables vi-mode editing. |

### EXTENDED DESCRIPTION > Command Line Editing (vi-mode)

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-110-13-03-001` | Verify that: In vi editing mode, there **must** be a distinguished line, the edit line. All the editing operations which modify a line affect the edit line. The edit line is always the newest line in the command history buffer. |
| `SHALL-20-110-13-03-002` | Verify that: When in insert mode, an entered character **must** be inserted into the command line, except as noted in vi Line Editing Insert Mode. Upon entering sh and after termination of the previous command, sh **must** be in insert mode. |
| `SHALL-20-110-13-03-003` | Verify that: Typing an escape character **must** switch sh into command mode (see vi Line Editing Command Mode). In command mode, an entered character **must** either invoke a defined operation, be used as part of a multi-character operation, or be treated as an error. A character that is not recognized as part of an editing command **must** terminate any specific editing command and **must** alert the terminal. If sh receives a SIGINT signal in command mode (whether generated by typing the interrupt character or by other means), it **must** terminate command line editing on the current command line, reissue the prompt on the next line of the terminal, and reset the command history (see fc) so that the most recently executed command is the previous command (that is, the command that was being edited when it was interrupted is not re-entered into the history). |
| `SHALL-20-110-13-03-004` | Verify that: In the following sections, the phrase "move the cursor to the beginning of the word" **must** mean "move the cursor to the first character of the current word" and the phrase "move the cursor to the end of the word" **must** mean "move the cursor to the last character of the current word". The phrase "beginning of the command line" indicates the point between the end of the prompt string issued by the shell (or the beginning of the terminal line, if there is no prompt string) and the first character of the command text. |

### EXTENDED DESCRIPTION > vi Line Editing Insert Mode

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-110-13-04-001` | Verify that: While in insert mode, any character typed **must** be inserted in the current command line, unless it is from the following set. |
| `SHALL-20-110-13-04-002` | Verify that: Execute the current command line. If the current command line is not empty, this line **must** be entered into the command history (see fc). |
| `SHALL-20-110-13-04-003` | Verify that: Delete the character previous to the current cursor position and move the current cursor position back one character. In insert mode, characters **must** be erased from both the screen and the buffer when backspacing. |
| `SHALL-20-110-13-04-004` | Verify that: If sh receives a SIGINT signal in insert mode (whether generated by typing the interrupt character or by other means), it **must** terminate command line editing with the same effects as described for interrupting command mode; see Command Line Editing (vi-mode). |
| `SHALL-20-110-13-04-005` | Verify that: Interpreted as the end of input in sh. This interpretation **must** occur only at the beginning of an input line. If end-of-file is entered other than at the beginning of the line, the results are unspecified. |

### EXTENDED DESCRIPTION > vi Line Editing Command Mode

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-110-13-05-001` | Verify that: In command mode for the command line editing feature, decimal digits not beginning with 0 that precede a command letter **must** be remembered. Some commands use these decimal digits as a count number that affects the operation. |
| `SHALL-20-110-13-05-002` | Verify that: If the current line is not the edit line, any command that modifies the current line **must** cause the content of the current line to replace the content of the edit line, and the current line **must** become the edit line. This replacement cannot be undone (see the u and U commands below). The modification requested **must** then be performed to the edit line. When the current line is the edit line, the modification **must** be done directly to the edit line. |
| `SHALL-20-110-13-05-003` | Verify that: Any command that is preceded by count **must** take a count (the numeric value of any preceding decimal digits). Unless otherwise noted, this count **must** cause the specified operation to repeat by the number of times specified by the count. Also unless otherwise noted, a count that is out of range is considered an error condition and **must** alert the terminal, but neither the cursor position, nor the command line, **must** change. |
| `SHALL-20-110-13-05-004` | Verify that: The following commands **must** be recognized in command mode: |
| `SHALL-20-110-13-05-005` | Verify that: Execute the current command line. If the current command line is not empty, this line **must** be entered into the command history (see fc). |
| `SHALL-20-110-13-05-006` | Verify that: Insert the character '#' at the beginning of the current command line and treat the resulting edit line as a comment. This line **must** be entered into the command history; see fc. |
| `SHALL-20-110-13-05-007` | Verify that: These expansions **must** be displayed on subsequent terminal lines. If the bigword contains none of the characters '?', '*', or '[', an <asterisk> ('*') **must** be implicitly assumed at the end. If any directories are matched, these expansions **must** have a '/' character appended. After the expansion, the line **must** be redrawn, the cursor repositioned at the current cursor position, and sh **must** be placed in command mode. |
| `SHALL-20-110-13-05-008` | Verify that: Perform pathname expansion (see 2.6.6 Pathname Expansion) on the current bigword, up to the largest set of characters that can be matched uniquely. If the bigword contains none of the characters '?', '*', or '[', an <asterisk> ('*') **must** be implicitly assumed at the end. This maximal expansion then **must** replace the original bigword in the command line, and the cursor **must** be placed after this expansion. If the resulting bigword completely and uniquely matches a directory, a '/' character **must** be inserted directly after the bigword. If some other file is completely matched, a single <space> **must** be inserted after the bigword. After this operation, sh **must** be placed in insert mode. |
| `SHALL-20-110-13-05-009` | Verify that: Perform pathname expansion on the current bigword and insert all expansions into the command to replace the current bigword, with each expansion separated by a single <space>. If at the end of the line, the current cursor position **must** be moved to the first column position following the expansions and sh **must** be placed in insert mode. Otherwise, the current cursor position **must** be the last column position of the first character after the expansions and sh **must** be placed in insert mode. If the current bigword contains none of the characters '?', '*', or '[', before the operation, an <asterisk> ('*') **must** be implicitly assumed at the end. |
| `SHALL-20-110-13-05-010` | Verify that: Insert the value of the alias named _letter. The symbol letter represents a single alphabetic character from the portable character set; implementations may support additional characters as an extension. If the alias _letter contains other editing commands, these commands **must** be performed as part of the insertion. If no alias _letter is enabled, this command **must** have no effect. |
| `SHALL-20-110-13-05-011` | Verify that: Convert, if the current character is a lowercase letter, to the equivalent uppercase letter and vice versa, as prescribed by the current locale. The current cursor position then **must** be advanced by one character. If the cursor was positioned on the last character of the line, the case conversion **must** occur, but the cursor **must** not advance. If the '~' command is preceded by a count, that number of characters **must** be converted, and the cursor **must** be advanced to the character position after the last character converted. If the count is larger than the number of characters after the cursor, this **must** not be considered an error; the cursor **must** advance to the last character on the line. |
| `SHALL-20-110-13-05-012` | Verify that: Repeat the most recent non-motion command, even if it was executed on an earlier command line. If the previous command was preceded by a count, and no count is given on the '.' command, the count from the previous command **must** be included as part of the repeated command. If the '.' command is preceded by a count, this **must** override any count argument to the previous command. The count specified in the '.' command **must** become the count for subsequent '.' commands issued without a count. |
| `SHALL-20-110-13-05-013` | Verify that: Invoke the vi editor to edit the current command line in a temporary file. When the editor exits, the commands in the temporary file **must** be executed and placed in the command history. If a number is included, it specifies the command number in the command history to be edited, rather than the current command line. |
| `SHALL-20-110-13-05-014` | Verify that: Move the current cursor position to the next character position. If the cursor was positioned on the last character of the line, the terminal **must** be alerted and the cursor **must** not be advanced. If the count is larger than the number of characters after the cursor, this **must** not be considered an error; the cursor **must** advance to the last character on the line. |
| `SHALL-20-110-13-05-015` | Verify that: Move the current cursor position to the countth (default 1) previous character position. If the cursor was positioned on the first character of the line, the terminal **must** be alerted and the cursor **must** not be moved. If the count is larger than the number of characters before the cursor, this **must** not be considered an error; the cursor **must** move to the first character on the line. |
| `SHALL-20-110-13-05-016` | Verify that: Move to the start of the next word. If the cursor was positioned on the last character of the line, the terminal **must** be alerted and the cursor **must** not be advanced. If the count is larger than the number of words after the cursor, this **must** not be considered an error; the cursor **must** advance to the last character on the line. |
| `SHALL-20-110-13-05-017` | Verify that: Move to the start of the next bigword. If the cursor was positioned on the last character of the line, the terminal **must** be alerted and the cursor **must** not be advanced. If the count is larger than the number of bigwords after the cursor, this **must** not be considered an error; the cursor **must** advance to the last character on the line. |
| `SHALL-20-110-13-05-018` | Verify that: Move to the end of the current word. If at the end of a word, move to the end of the next word. If the cursor was positioned on the last character of the line, the terminal **must** be alerted and the cursor **must** not be advanced. If the count is larger than the number of words after the cursor, this **must** not be considered an error; the cursor **must** advance to the last character on the line. |
| `SHALL-20-110-13-05-019` | Verify that: Move to the end of the current bigword. If at the end of a bigword, move to the end of the next bigword. If the cursor was positioned on the last character of the line, the terminal **must** be alerted and the cursor **must** not be advanced. If the count is larger than the number of bigwords after the cursor, this **must** not be considered an error; the cursor **must** advance to the last character on the line. |
| `SHALL-20-110-13-05-020` | Verify that: Move to the beginning of the current word. If at the beginning of a word, move to the beginning of the previous word. If the cursor was positioned on the first character of the line, the terminal **must** be alerted and the cursor **must** not be moved. If the count is larger than the number of words preceding the cursor, this **must** not be considered an error; the cursor **must** return to the first character on the line. |
| `SHALL-20-110-13-05-021` | Verify that: Move to the beginning of the current bigword. If at the beginning of a bigword, move to the beginning of the previous bigword. If the cursor was positioned on the first character of the line, the terminal **must** be alerted and the cursor **must** not be moved. If the count is larger than the number of bigwords preceding the cursor, this **must** not be considered an error; the cursor **must** return to the first character on the line. |
| `SHALL-20-110-13-05-022` | Verify that: Move to the countth character position on the current command line. If no number is specified, move to the first position. The first character position **must** be numbered 1. If the count is larger than the number of characters on the line, this **must** not be considered an error; the cursor **must** be placed on the last character on the line. |
| `SHALL-20-110-13-05-023` | Verify that: Move to the first occurrence of the character 'c' that occurs after the current cursor position. If the cursor was positioned on the last character of the line, the terminal **must** be alerted and the cursor **must** not be advanced. If the character 'c' does not occur in the line after the current cursor position, the terminal **must** be alerted and the cursor **must** not be moved. |
| `SHALL-20-110-13-05-024` | Verify that: Move to the first occurrence of the character 'c' that occurs before the current cursor position. If the cursor was positioned on the first character of the line, the terminal **must** be alerted and the cursor **must** not be moved. If the character 'c' does not occur in the line before the current cursor position, the terminal **must** be alerted and the cursor **must** not be moved. |
| `SHALL-20-110-13-05-025` | Verify that: Move to the character before the first occurrence of the character 'c' that occurs after the current cursor position. If the cursor was positioned on the last character of the line, the terminal **must** be alerted and the cursor **must** not be advanced. If the character 'c' does not occur in the line after the current cursor position, the terminal **must** be alerted and the cursor **must** not be moved. |
| `SHALL-20-110-13-05-026` | Verify that: Move to the character after the first occurrence of the character 'c' that occurs before the current cursor position. If the cursor was positioned on the first character of the line, the terminal **must** be alerted and the cursor **must** not be moved. If the character 'c' does not occur in the line before the current cursor position, the terminal **must** be alerted and the cursor **must** not be moved. |
| `SHALL-20-110-13-05-027` | Verify that: Repeat the most recent f, F, t, or T command. Any number argument on that previous command **must** be ignored. Errors are those described for the repeated command. |
| `SHALL-20-110-13-05-028` | Verify that: Repeat the most recent f, F, t, or T command. Any number argument on that previous command **must** be ignored. However, reverse the direction of that command. |
| `SHALL-20-110-13-05-029` | Verify that: Enter insert mode after the current cursor position. Characters that are entered **must** be inserted before the next character. |
| `SHALL-20-110-13-05-030` | Verify that: Enter insert mode at the current cursor position. Characters that are entered **must** be inserted before the current character. |
| `SHALL-20-110-13-05-031` | Verify that: If the motion command is the character 'c', the current command line **must** be cleared and insert mode **must** be entered. If the motion command would move the current cursor position toward the beginning of the command line, the character under the current cursor position **must** not be deleted. If the motion command would move the current cursor position toward the end of the command line, the character under the current cursor position **must** be deleted. If the count is larger than the number of characters between the current cursor position and the end of the command line toward which the motion command would move the cursor, this **must** not be considered an error; all of the remaining characters in the aforementioned range **must** be deleted and insert mode **must** be entered. If the motion command is invalid, the terminal **must** be alerted, the cursor **must** not be moved, and no text **must** be deleted. |
| `SHALL-20-110-13-05-032` | Verify that: Replace the current character with the character 'c'. With a number count, replace the current and the following count-1 characters. After this command, the current cursor position **must** be on the last character that was changed. If the count is larger than the number of characters after the cursor, this **must** not be considered an error; all of the remaining characters **must** be changed. |
| `SHALL-20-110-13-05-033` | Verify that: Delete the character at the current cursor position and place the deleted characters in the save buffer. If the cursor was positioned on the last character of the line, the character **must** be deleted and the cursor position **must** be moved to the previous character (the new last character). If the count is larger than the number of characters after the cursor, this **must** not be considered an error; all the characters from the cursor to the end of the line **must** be deleted. |
| `SHALL-20-110-13-05-034` | Verify that: Delete the character before the current cursor position and place the deleted characters in the save buffer. The character under the current cursor position **must** not change. If the cursor was positioned on the first character of the line, the terminal **must** be alerted, and the X command **must** have no effect. If the line contained a single character, the X command **must** have no effect. If the line contained no characters, the terminal **must** be alerted and the cursor **must** not be moved. If the count is larger than the number of characters before the cursor, this **must** not be considered an error; all the characters from before the cursor to the beginning of the line **must** be deleted. |
| `SHALL-20-110-13-05-035` | Verify that: Delete the characters between the current cursor position and the character position that would result from the motion command. A number count repeats the motion command count times. If the motion command would move toward the beginning of the command line, the character under the current cursor position **must** not be deleted. If the motion command is d, the entire current command line **must** be cleared. If the count is larger than the number of characters between the current cursor position and the end of the command line toward which the motion command would move the cursor, this **must** not be considered an error; all of the remaining characters in the aforementioned range **must** be deleted. The deleted characters **must** be placed in the save buffer. |
| `SHALL-20-110-13-05-036` | Verify that: Delete all characters from the current cursor position to the end of the line. The deleted characters **must** be placed in the save buffer. |
| `SHALL-20-110-13-05-037` | Verify that: Yank (that is, copy) the characters from the current cursor position to the position resulting from the motion command into the save buffer. A number count **must** be applied to the motion command. If the motion command would move toward the beginning of the command line, the character under the current cursor position **must** not be included in the set of yanked characters. If the motion command is y, the entire current command line **must** be yanked into the save buffer. The current cursor position **must** be unchanged. If the count is larger than the number of characters between the current cursor position and the end of the command line toward which the motion command would move the cursor, this **must** not be considered an error; all of the remaining characters in the aforementioned range **must** be yanked. |
| `SHALL-20-110-13-05-038` | Verify that: Yank the characters from the current cursor position to the end of the line into the save buffer. The current character position **must** be unchanged. |
| `SHALL-20-110-13-05-039` | Verify that: Put a copy of the current contents of the save buffer after the current cursor position. The current cursor position **must** be advanced to the last character put from the save buffer. A count **must** indicate how many copies of the save buffer **must** be put. |
| `SHALL-20-110-13-05-040` | Verify that: Put a copy of the current contents of the save buffer before the current cursor position. The current cursor position **must** be moved to the last character put from the save buffer. A count **must** indicate how many copies of the save buffer **must** be put. |
| `SHALL-20-110-13-05-041` | Verify that: Undo the last command that changed the edit line. This operation **must** not undo the copy of any command line to the edit line. |
| `SHALL-20-110-13-05-042` | Verify that: Undo all changes made to the edit line. This operation **must** not undo the copy of any command line to the edit line. |
| `SHALL-20-110-13-05-043` | Verify that: Set the current command line to be the countth previous command line in the shell command history. If count is not specified, it **must** default to 1. The cursor **must** be positioned on the first character of the new command. If a k or - command would retreat past the maximum number of commands in effect for this shell (affected by the HISTSIZE environment variable), the terminal **must** be alerted, and the command **must** have no effect. |
| `SHALL-20-110-13-05-044` | Verify that: Set the current command line to be the countth next command line in the shell command history. If count is not specified, it **must** default to 1. The cursor **must** be positioned on the first character of the new command. If a j or + command advances past the edit line, the current command line **must** be restored to the edit line and the terminal **must** be alerted. |
| `SHALL-20-110-13-05-045` | Verify that: Set the current command line to be the oldest command line stored in the shell command history. With a number number, set the current command line to be the command line number in the history. If command line number does not exist, the terminal **must** be alerted and the command line **must** not be changed. |
| `SHALL-20-110-13-05-046` | Verify that: If pattern is empty, the last non-empty pattern provided to / or ? **must** be used. If there is no previous non-empty pattern, the terminal **must** be alerted and the current command line **must** remain unchanged. |
| `SHALL-20-110-13-05-047` | Verify that: If pattern is empty, the last non-empty pattern provided to / or ? **must** be used. If there is no previous non-empty pattern, the terminal **must** be alerted and the current command line **must** remain unchanged. |
| `SHALL-20-110-13-05-048` | Verify that: Repeat the most recent / or ? command. If there is no previous / or ?, the terminal **must** be alerted and the current command line **must** remain unchanged. |
| `SHALL-20-110-13-05-049` | Verify that: Repeat the most recent / or ? command, reversing the direction of the search. If there is no previous / or ?, the terminal **must** be alerted and the current command line **must** remain unchanged. |

### EXIT STATUS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-110-14-001` | Verify that: The following exit values **must** be returned: |
| `SHALL-20-110-14-002` | Verify that: Otherwise, the shell **must** terminate in the same manner as for an exit command with no operands, unless the last command the shell invoked was executed without forking, in which case the wait status seen by the parent process of the shell **must** be the wait status of the last command the shell invoked. See the exit utility in 2.15 Special Built-In Utilities. |

### RATIONALE

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-110-18-001` | Verify that: If followed by the erase or kill character, that character **must** be inserted into the input line. Otherwise, the <backslash> itself **must** be inserted into the input line. |

## alias (Built-in)

### DESCRIPTION

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-02-03-001` | Verify that: The alias utility **must** create or redefine alias definitions or write the values of existing alias definitions to standard output. An alias definition provides a string value that **must** replace a command name when it is encountered. For information on valid string values, and the processing involved, see 2.3.1 Alias Substitution. |

### OPERANDS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-02-05-001` | Verify that: The following operands **must** be supported: |
| `SHALL-20-02-05-002` | Verify that: If no operands are given, all alias definitions **must** be written to standard output. |

### ENVIRONMENT VARIABLES

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-02-08-001` | Verify that: The following environment variables **must** affect the execution of alias: |

### STDOUT

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-02-10-001` | Verify that: The format for displaying aliases (when no operands or only name operands are specified) **must** be: |
| `SHALL-20-02-10-002` | Verify that: The value string **must** be written with appropriate quoting so that it is suitable for reinput to the shell. See the description of shell quoting in 2.2 Quoting. |

### EXIT STATUS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-02-14-001` | Verify that: The following exit values **must** be returned: |

## bg (Built-in)

### DESCRIPTION

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-10-03-001` | Verify that: If job control is enabled (see the description of set -m), the shell is interactive, and the current shell execution environment (see 2.13 Shell Execution Environment) is not a subshell environment, the bg utility **must** resume suspended jobs from the current shell execution environment by running them as background jobs, as described in 2.11 Job Control; it may also do so if the shell is non-interactive or the current shell execution environment is a subshell environment. If the job specified by job_id is already a running background job, the bg utility **must** have no effect and **must** exit successfully. |

### OPERANDS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-10-05-001` | Verify that: The following operand **must** be supported: |
| `SHALL-20-10-05-002` | Verify that: Specify the job to be resumed as a background job. If no job_id operand is given, the most recently suspended job **must** be used. The format of job_id is described in XBD 3.182 Job ID . |

### ENVIRONMENT VARIABLES

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-10-08-001` | Verify that: The following environment variables **must** affect the execution of bg: |

### STDOUT

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-10-10-001` | Verify that: The output of bg **must** consist of a line in the format: |

### EXIT STATUS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-10-14-001` | Verify that: The following exit values **must** be returned: |

### CONSEQUENCES OF ERRORS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-10-15-001` | Verify that: If job control is disabled, the bg utility **must** exit with an error and no job **must** be placed in the background. |

## cd (Built-in)

### DESCRIPTION

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-14-03-001` | Verify that: The cd utility **must** change the working directory of the current shell execution environment (see 2.13 Shell Execution Environment) by executing the following steps in sequence. (In the following steps, the symbol curpath represents an intermediate value used to simplify the description of the algorithm used by cd. There is no requirement that curpath be made visible to the application.) |
| `SHALL-20-14-03-002` | Verify that: If no directory operand is given and the HOME environment variable is empty or undefined, the default behavior is implementation-defined and no further steps **must** be taken. |
| `SHALL-20-14-03-003` | Verify that: If no directory operand is given and the HOME environment variable is set to a non-empty value, the cd utility **must** behave as if the directory named in the HOME environment variable was specified as the directory operand. |
| `SHALL-20-14-03-004` | Verify that: The curpath value **must** then be converted to canonical form as follows, considering each component from beginning to end, in sequence: |
| `SHALL-20-14-03-005` | Verify that: Dot components and any <slash> characters that separate them from the next component **must** be deleted. |
| `SHALL-20-14-03-006` | Verify that: If the preceding component does not refer (in the context of pathname resolution with symbolic links followed) to a directory, then the cd utility **must** display an appropriate error message and no further steps **must** be taken. |
| `SHALL-20-14-03-007` | Verify that: The preceding component, all <slash> characters separating the preceding component from dot-dot, dot-dot, and all <slash> characters separating dot-dot from the following component (if any) **must** be deleted. |
| `SHALL-20-14-03-008` | Verify that: An implementation may further simplify curpath by removing any trailing <slash> characters that are not also leading <slash> characters, replacing multiple non-leading consecutive <slash> characters with a single <slash>, and replacing three or more leading <slash> characters with a single <slash>. If, as a result of this canonicalization, the curpath variable is null, no further steps **must** be taken. |
| `SHALL-20-14-03-009` | Verify that: If curpath is longer than {PATH_MAX} bytes (including the terminating null) and the directory operand was not longer than {PATH_MAX} bytes (including the terminating null), then curpath **must** be converted from an absolute pathname to an equivalent relative pathname if possible. This conversion **must** always be considered possible if the value of PWD , with a trailing <slash> added if it does not already have one, is an initial substring of curpath. Whether or not it is considered possible under other circumstances is unspecified. Implementations may also apply this conversion if curpath is not longer than {PATH_MAX} bytes or the directory operand was longer than {PATH_MAX} bytes. |
| `SHALL-20-14-03-010` | Verify that: The cd utility **must** then perform actions equivalent to the chdir() function called with curpath as the path argument. If these actions fail for any reason, the cd utility **must** display an appropriate error message and the remainder of this step **must** not be executed. If the -P option is not in effect, the PWD environment variable **must** be set to the value that curpath had on entry to step 9 (i.e., before conversion to a relative pathname). |
| `SHALL-20-14-03-011` | Verify that: If the -P option is in effect, the PWD environment variable **must** be set to the string that would be output by pwd -P. If there is insufficient permission on the new directory, or on any parent of that directory, to determine the current working directory, the value of the PWD environment variable is unspecified. If both the -e and the -P options are in effect and cd is unable to determine the pathname of the current working directory, cd **must** complete successfully but return a non-zero exit status. |
| `SHALL-20-14-03-012` | Verify that: If, during the execution of the above steps, the PWD environment variable is set, the OLDPWD shell variable **must** also be set to the value of the old working directory (that is the current working directory immediately prior to the call to cd). It is unspecified whether, when setting OLDPWD , the shell also causes it to be exported if it was not already. |

### OPTIONS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-14-04-001` | Verify that: The cd utility **must** conform to XBD 12.2 Utility Syntax Guidelines. |
| `SHALL-20-14-04-002` | Verify that: The following options **must** be supported by the implementation: |
| `SHALL-20-14-04-003` | Verify that: Handle the operand dot-dot logically; symbolic link components **must** not be resolved before dot-dot components are processed (see steps 8. and 9. in the DESCRIPTION). |
| `SHALL-20-14-04-004` | Verify that: Handle the operand dot-dot physically; symbolic link components **must** be resolved before dot-dot components are processed (see step 7. in the DESCRIPTION). |
| `SHALL-20-14-04-005` | Verify that: If both -L and -P options are specified, the last of these options **must** be used and all others ignored. If neither -L nor -P is specified, the operand **must** be handled dot-dot logically; see the DESCRIPTION. |

### OPERANDS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-14-05-001` | Verify that: The following operands **must** be supported: |
| `SHALL-20-14-05-002` | Verify that: An absolute or relative pathname of the directory that **must** become the new working directory. The interpretation of a relative pathname by cd depends on the -L option and the CDPATH and PWD environment variables. If directory is an empty string, cd **must** write a diagnostic message to standard error and exit with non-zero status. If directory consists of a single '-' (<hyphen-minus>) character, the cd utility **must** behave as if directory contained the value of the OLDPWD environment variable, except that after it sets the value of PWD it **must** write the new value to standard output. The behavior is unspecified if OLDPWD does not start with a <slash> character. |

### ENVIRONMENT VARIABLES

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-14-08-001` | Verify that: The following environment variables **must** affect the execution of cd: |
| `SHALL-20-14-08-002` | Verify that: A <colon>-separated list of pathnames that refer to directories. The cd utility **must** use this list in its attempt to change the directory, as described in the DESCRIPTION. An empty string in place of a directory pathname represents the current directory. If CDPATH is not set, it **must** be treated as if it were an empty string. |
| `SHALL-20-14-08-003` | Verify that: This variable **must** be set as specified in the DESCRIPTION. If an application sets or unsets the value of PWD , the behavior of cd is unspecified. |

### STDOUT

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-14-10-001` | Verify that: If a non-empty directory name from CDPATH is not used, and the directory argument is not '-', there **must** be no output. |

### EXIT STATUS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-14-14-001` | Verify that: The following exit values **must** be returned: |

### CONSEQUENCES OF ERRORS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-14-15-001` | Verify that: The working directory **must** remain unchanged. |

## command (Built-in)

### DESCRIPTION

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-22-03-001` | Verify that: The command utility **must** cause the shell to treat the arguments as a simple command, suppressing the shell function lookup that is described in 2.9.1.4 Command Search and Execution, item 1c. |
| `SHALL-20-22-03-002` | Verify that: If the command_name is the same as the name of one of the special built-in utilities, the special properties in the enumerated list at the beginning of 2.15 Special Built-In Utilities **must** not occur. In every other respect, if command_name is not the name of a function, the effect of command (with no options) **must** be the same as omitting command, except that command_name does not appear in the command word position in the command command, and consequently is not subject to alias substitution (see 2.3.1 Alias Substitution) nor recognized as a reserved word (see 2.4 Reserved Words). |
| `SHALL-20-22-03-003` | Verify that: When the -v or -V option is used, the command utility **must** provide information concerning how a command name is interpreted by the shell. |
| `SHALL-20-22-03-004` | Verify that: The command utility **must** be treated as a declaration utility if the first argument passed to the utility is recognized as a declaration utility. In this case, subsequent words of the form name=word **must** be expanded in an assignment context. See 2.9.1.1 Order of Processing. |

### OPTIONS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-22-04-001` | Verify that: The command utility **must** conform to XBD 12.2 Utility Syntax Guidelines. |
| `SHALL-20-22-04-002` | Verify that: The following options **must** be supported: |
| `SHALL-20-22-04-003` | Verify that: Executable utilities, regular built-in utilities, command_names including a <slash> character, and any implementation-provided functions that are found using the PATH variable (as described in 2.9.1.4 Command Search and Execution), **must** be written as absolute pathnames. |
| `SHALL-20-22-04-004` | Verify that: Shell functions, special built-in utilities, regular built-in utilities not associated with a PATH search, and shell reserved words **must** be written as just their names. |
| `SHALL-20-22-04-005` | Verify that: An alias **must** be written as a command line that represents its alias definition. |
| `SHALL-20-22-04-006` | Verify that: Otherwise, no output **must** be written and the exit status **must** reflect that the name was not found. |
| `SHALL-20-22-04-007` | Verify that: Executable utilities, regular built-in utilities, and any implementation-provided functions that are found using the PATH variable (as described in 2.9.1.4 Command Search and Execution), **must** be identified as such and include the absolute pathname in the string. |
| `SHALL-20-22-04-008` | Verify that: Other shell functions **must** be identified as functions. |
| `SHALL-20-22-04-009` | Verify that: Aliases **must** be identified as aliases and their definitions included in the string. |
| `SHALL-20-22-04-010` | Verify that: Special built-in utilities **must** be identified as special built-in utilities. |
| `SHALL-20-22-04-011` | Verify that: Regular built-in utilities not associated with a PATH search **must** be identified as regular built-in utilities. (The term "regular" need not be used.) |
| `SHALL-20-22-04-012` | Verify that: Shell reserved words **must** be identified as reserved words. |

### OPERANDS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-22-05-001` | Verify that: The following operands **must** be supported: |

### ENVIRONMENT VARIABLES

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-22-08-001` | Verify that: The following environment variables **must** affect the execution of command: |

### STDOUT

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-22-10-001` | Verify that: When the -v option is specified, standard output **must** be formatted as: |
| `SHALL-20-22-10-002` | Verify that: When the -V option is specified, standard output **must** be formatted as: |

### EXIT STATUS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-22-14-001` | Verify that: When the -v or -V options are specified, the following exit values **must** be returned: |
| `SHALL-20-22-14-002` | Verify that: Otherwise, the following exit values **must** be returned: |
| `SHALL-20-22-14-003` | Verify that: Otherwise, the exit status of command **must** be that of the simple command specified by the arguments to command. |

## fc (Built-in)

### DESCRIPTION

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-44-03-001` | Verify that: The fc utility **must** list, or **must** edit and re-execute, commands previously entered to an interactive sh. |
| `SHALL-20-44-03-002` | Verify that: The command history list **must** reference commands by number. The first number in the list is selected arbitrarily. The relationship of a number to its command **must** not change except when the user logs in and no other process is accessing the list, at which time the system may reset the numbering to start the oldest retained command at another number (usually 1). When the number reaches an implementation-defined upper limit, which **must** be no smaller than the value in HISTSIZE or 32767 (whichever is greater), the shell may wrap the numbers, starting the next command with a lower number (usually 1). However, despite this optional wrapping of numbers, fc **must** maintain the time-ordering sequence of the commands. For example, if four commands in sequence are given the numbers 32766, 32767, 1 (wrapped), and 2 as they are executed, command 32767 is considered the command previous to 1, even though its number is higher. |
| `SHALL-20-44-03-003` | Verify that: When commands are edited (when the -l option is not specified), the resulting lines **must** be entered at the end of the history list and then re-executed by sh. The fc command that caused the editing **must** not be entered into the history list. If the editor returns a non-zero exit status, this **must** suppress the entry into the history list and the command re-execution. Any command line variable assignments or redirection operators used with fc **must** affect both the fc command itself as well as the command that results; for example: |

### OPTIONS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-44-04-001` | Verify that: The fc utility **must** conform to XBD 12.2 Utility Syntax Guidelines. |
| `SHALL-20-44-04-002` | Verify that: The following options **must** be supported: |
| `SHALL-20-44-04-003` | Verify that: Use the editor named by editor to edit the commands. The editor string is a utility name, subject to search via the PATH variable (see XBD 8. Environment Variables). The value in the FCEDIT variable **must** be used as a default when -e is not specified. If FCEDIT is null or unset, ed **must** be used as the editor. |
| `SHALL-20-44-04-004` | Verify that: (The letter ell.) List the commands rather than invoking an editor on them. The commands **must** be written in the sequence indicated by the first and last operands, as affected by -r, with each command preceded by the command number. |

### OPERANDS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-44-05-001` | Verify that: The following operands **must** be supported: |
| `SHALL-20-44-05-002` | Verify that: If first is omitted, the previous command **must** be used. |
| `SHALL-20-44-05-003` | Verify that: If last is omitted, last **must** default to the previous command when -l is specified; otherwise, it **must** default to first. |
| `SHALL-20-44-05-004` | Verify that: If first and last are both omitted, the previous 16 commands **must** be listed or the previous single command **must** be edited (based on the -l option). |
| `SHALL-20-44-05-005` | Verify that: If first and last are both present, all of the commands from first to last **must** be edited (without -l) or listed (with -l). Editing multiple commands **must** be accomplished by presenting to the editor all of the commands at one time, each command starting on a new line. If first represents a newer command than last, the commands **must** be listed or edited in reverse sequence, equivalent to using -r. For example, the following commands on the first line are equivalent to the corresponding commands on the second: |
| `SHALL-20-44-05-006` | Verify that: When a range of commands is used, it **must** not be an error to specify first or last values that are not in the history list; fc **must** substitute the value representing the oldest or newest command in the list, as appropriate. For example, if there are only ten commands in the history list, numbered 1 to 10: |
| `SHALL-20-44-05-007` | Verify that: **must** list and edit, respectively, all ten commands. |

### ENVIRONMENT VARIABLES

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-44-08-001` | Verify that: The following environment variables **must** affect the execution of fc: |
| `SHALL-20-44-08-002` | Verify that: This variable, when expanded by the shell, **must** determine the default value for the -e editor option's editor option-argument. If FCEDIT is null or unset, ed **must** be used as the editor. |
| `SHALL-20-44-08-003` | Verify that: Determine a decimal number representing the limit to the number of previous commands that are accessible. If this variable is unset, an unspecified default greater than or equal to 128 **must** be used. The maximum number of commands in the history list is unspecified, but **must** be at least 128. An implementation may choose to access this variable only when initializing the history file, as described under HISTFILE . Therefore, it is unspecified whether changes made to HISTSIZE after the history file has been initialized are effective. |

### STDOUT

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-44-10-001` | Verify that: When the -l option is used to list commands, the format of each command in the list **must** be as follows: |
| `SHALL-20-44-10-002` | Verify that: If both the -l and -n options are specified, the format of each command **must** be: |
| `SHALL-20-44-10-003` | Verify that: If the <command> consists of more than one line, the lines after the first **must** be displayed as: |

### EXIT STATUS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-44-14-001` | Verify that: The following exit values **must** be returned: |
| `SHALL-20-44-14-002` | Verify that: Otherwise, the exit status **must** be that of the commands executed by fc. |

## fg (Built-in)

### DESCRIPTION

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-45-03-001` | Verify that: If job control is enabled (see the description of set -m), the shell is interactive, and the current shell execution environment (see 2.13 Shell Execution Environment) is not a subshell environment, the fg utility **must** move a background job in the current execution environment into the foreground, as described in 2.11 Job Control; it may also do so if the shell is non-interactive or the current shell execution environment is a subshell environment. |
| `SHALL-20-45-03-002` | Verify that: Using fg to place a job into the foreground **must** remove its process ID from the list of those "known in the current shell execution environment"; see 2.9.3.1 Asynchronous AND-OR Lists. |

### OPERANDS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-45-05-001` | Verify that: The following operand **must** be supported: |
| `SHALL-20-45-05-002` | Verify that: Specify the job to be run as a foreground job. If no job_id operand is given, the job_id for the job that was most recently suspended, placed in the background, or run as a background job **must** be used. The format of job_id is described in XBD 3.182 Job ID. |

### ENVIRONMENT VARIABLES

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-45-08-001` | Verify that: The following environment variables **must** affect the execution of fg: |

### STDOUT

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-45-10-001` | Verify that: The fg utility **must** write the command line of the job to standard output in the following format: |

### EXIT STATUS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-45-14-001` | Verify that: If fg does not move a job into the foreground, the following exit value **must** be returned: |

### CONSEQUENCES OF ERRORS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-45-15-001` | Verify that: If job control is disabled, the fg utility **must** exit with an error and no job **must** be placed in the foreground. |

## getopts (Built-in)

### DESCRIPTION

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-53-03-001` | Verify that: The getopts utility **must** retrieve options and option-arguments from a list of parameters. It **must** support the Utility Syntax Guidelines 3 to 10, inclusive, described in XBD 12.2 Utility Syntax Guidelines. |
| `SHALL-20-53-03-002` | Verify that: When the shell is first invoked, the shell variable OPTIND **must** be initialized to 1. Each time getopts is invoked, it **must** place the value of the next option found in the parameter list in the shell variable specified by the name operand and the shell variable OPTIND **must** be set as follows: |
| `SHALL-20-53-03-003` | Verify that: When getopts successfully parses an option that takes an option-argument (that is, a character followed by <colon> in optstring, and exit status is 0), the value of OPTIND **must** be the integer index of the next element of the parameter list (if any; see OPERANDS below) to be searched for an option character. Index 1 identifies the first element of the parameter list. |
| `SHALL-20-53-03-004` | Verify that: When getopts reports end of options (that is, when exit status is 1), the value of OPTIND **must** be the integer index of the next element of the parameter list (if any). |
| `SHALL-20-53-03-005` | Verify that: In all other cases, the value of OPTIND is unspecified, but **must** encode the information needed for the next invocation of getopts to resume parsing options after the option just parsed. |
| `SHALL-20-53-03-006` | Verify that: When the option requires an option-argument, the getopts utility **must** place it in the shell variable OPTARG . If no option was found, or if the option that was found does not have an option-argument, OPTARG **must** be unset. |
| `SHALL-20-53-03-007` | Verify that: If an option character not contained in the optstring operand is found where an option character is expected, the shell variable specified by name **must** be set to the <question-mark> ('?') character. In this case, if the first character in optstring is a <colon> (':'), the shell variable OPTARG **must** be set to the option character found, but no output **must** be written to standard error; otherwise, the shell variable OPTARG **must** be unset and a diagnostic message **must** be written to standard error. This condition **must** be considered to be an error detected in the way arguments were presented to the invoking application, but **must** not be an error in getopts processing. |
| `SHALL-20-53-03-008` | Verify that: If the first character of optstring is a <colon>, the shell variable specified by name **must** be set to the <colon> character and the shell variable OPTARG **must** be set to the option character found. |
| `SHALL-20-53-03-009` | Verify that: Otherwise, the shell variable specified by name **must** be set to the <question-mark> character, the shell variable OPTARG **must** be unset, and a diagnostic message **must** be written to standard error. This condition **must** be considered to be an error detected in the way arguments were presented to the invoking application, but **must** not be an error in getopts processing; a diagnostic message **must** be written as stated, but the exit status **must** be zero. |
| `SHALL-20-53-03-010` | Verify that: When the end of options is encountered, the getopts utility **must** exit with a return value of one; the shell variable OPTIND **must** be set to the index of the argument containing the first operand in the parameter list, or the value 1 plus the number of elements in the parameter list if there are no operands in the parameter list; the name variable **must** be set to the <question-mark> character. Any of the following **must** identify the end of options: the first "--" element of the parameter list that is not an option-argument, finding an element of the parameter list that is not an option-argument and does not begin with a '-', or encountering an error. |
| `SHALL-20-53-03-011` | Verify that: The shell variables OPTIND and OPTARG **must** not be exported by default. An error in setting any of these variables (such as if name has previously been marked readonly) **must** be considered an error of getopts processing, and **must** result in a return value greater than one. |

### OPERANDS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-53-05-001` | Verify that: The following operands **must** be supported: |
| `SHALL-20-53-05-002` | Verify that: A string containing the option characters recognized by the utility invoking getopts. If a character is followed by a <colon>, the option **must** be expected to have an argument, which should be supplied as a separate argument. Applications should specify an option character and its option-argument as separate arguments, but getopts **must** interpret the characters following an option character requiring arguments as an argument whether or not this is done. An explicit null option-argument need not be recognized if it is not supplied as a separate argument when getopts is invoked. (See also the getopt() function defined in the System Interfaces volume of POSIX.1-2024.) The characters <question-mark> and <colon> **must** not be used as option characters by an application. The use of other option characters that are not alphanumeric produces unspecified results. Whether or not the option-argument is supplied as a separate argument from the option character, the value in OPTARG **must** only be the characters of the option-argument. The first character in optstring determines how getopts behaves if an option character is not known or an option-argument is missing. |
| `SHALL-20-53-05-003` | Verify that: The name of a shell variable that **must** be set by the getopts utility to the option character that was found. |
| `SHALL-20-53-05-004` | Verify that: By default, the list of parameters parsed by the getopts utility **must** be the positional parameters currently set in the invoking shell environment ("$@"). If param operands are given, they **must** be parsed instead of the positional parameters. Note that the next element of the parameter list need not exist; in this case, OPTIND will be set to $#+1 or the number of param operands plus 1. |

### ENVIRONMENT VARIABLES

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-53-08-001` | Verify that: The following environment variables **must** affect the execution of getopts: |
| `SHALL-20-53-08-002` | Verify that: This variable **must** be used by the getopts utility as the index of the next argument to be processed. |

### STDERR

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-53-11-001` | Verify that: Whenever an error is detected and the first character in the optstring operand is not a <colon> (':'), a diagnostic message **must** be written to standard error with the following information in an unspecified format: |
| `SHALL-20-53-11-002` | Verify that: The invoking program name **must** be identified in the message. The invoking program name **must** be the value of the shell special parameter 0 (see 2.5.2 Special Parameters) at the time the getopts utility is invoked. A name equivalent to: |
| `SHALL-20-53-11-003` | Verify that: If an option is found that was not specified in optstring, this error is identified and the invalid option character **must** be identified in the message. |
| `SHALL-20-53-11-004` | Verify that: If an option requiring an option-argument is found, but an option-argument is not found, this error **must** be identified and the invalid option character **must** be identified in the message. |

### EXIT STATUS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-53-14-001` | Verify that: The following exit values **must** be returned: |

## hash (Built-in)

### DESCRIPTION

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-56-03-001` | Verify that: The hash utility **must** affect the way the current shell environment remembers the locations of utilities found as described in 2.9.1.4 Command Search and Execution. Depending on the arguments specified, it **must** add utility locations to its list of remembered locations or it **must** purge the contents of the list. When no arguments are specified, it **must** report on the contents of the list. |
| `SHALL-20-56-03-002` | Verify that: Utilities provided as built-ins to the shell and functions **must** not be reported by hash. |

### OPTIONS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-56-04-001` | Verify that: The hash utility **must** conform to XBD 12.2 Utility Syntax Guidelines. |
| `SHALL-20-56-04-002` | Verify that: The following option **must** be supported: |

### OPERANDS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-56-05-001` | Verify that: The following operand **must** be supported: |

### ENVIRONMENT VARIABLES

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-56-08-001` | Verify that: The following environment variables **must** affect the execution of hash: |

### EXIT STATUS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-56-14-001` | Verify that: The following exit values **must** be returned: |

## jobs (Built-in)

### DESCRIPTION

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-62-03-001` | Verify that: If the current shell execution environment (see 2.13 Shell Execution Environment) is not a subshell environment, the jobs utility **must** display the status of background jobs that were created in the current shell execution environment; it may also do so if the current shell execution environment is a subshell environment. |
| `SHALL-20-62-03-002` | Verify that: When jobs reports the termination status of a job, the shell **must** remove the job from the background jobs list and the associated process ID from the list of those "known in the current shell execution environment"; see 2.9.3.1 Asynchronous AND-OR Lists. If a write error occurs when jobs writes to standard output, some process IDs might have been removed from the list but not successfully reported. |

### OPTIONS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-62-04-001` | Verify that: The jobs utility **must** conform to XBD 12.2 Utility Syntax Guidelines. |
| `SHALL-20-62-04-002` | Verify that: The following options **must** be supported: |
| `SHALL-20-62-04-003` | Verify that: By default, the jobs utility **must** display the status of all background jobs, both running and suspended, and all jobs whose status has changed and have not been reported by the shell. |

### OPERANDS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-62-05-001` | Verify that: The following operand **must** be supported: |
| `SHALL-20-62-05-002` | Verify that: Specifies the jobs for which the status is to be displayed. If no job_id is given, the status information for all jobs **must** be displayed. The format of job_id is described in XBD 3.182 Job ID. |

### ENVIRONMENT VARIABLES

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-62-08-001` | Verify that: The following environment variables **must** affect the execution of jobs: |

### STDOUT

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-62-10-001` | Verify that: If the -p option is specified, the output **must** consist of one line for each process ID: |
| `SHALL-20-62-10-002` | Verify that: Otherwise, if the -l option is not specified, the output **must** be a series of lines of the form: |
| `SHALL-20-62-10-003` | Verify that: where the fields **must** be as follows: |
| `SHALL-20-62-10-004` | Verify that: The character '+' identifies the job that would be used as a default for the fg or bg utilities; this job can also be specified using the job_id %+ or "%%". The character '-' identifies the job that would become the default if the current default job were to exit; this job can also be specified using the job_id %-. For other jobs, this field is a <space>. At most one job can be identified with '+' and at most one job can be identified with '-'. If there is any suspended job, then the current job **must** be a suspended job. If there are at least two suspended jobs, then the previous job also **must** be a suspended job. |
| `SHALL-20-62-10-005` | Verify that: The implementation may substitute the string Suspended in place of Stopped. If the job was terminated by a signal, the format of <state> is unspecified, but it **must** be visibly distinct from all of the other <state> formats shown here and **must** indicate the name or description of the signal causing the termination. |
| `SHALL-20-62-10-006` | Verify that: For job-control background jobs, a field containing the process group ID **must** be inserted before the <state> field. Also, more processes in a process group may be output on separate lines, using only the process ID and <command> fields. |
| `SHALL-20-62-10-007` | Verify that: For non-job-control background jobs (if supported), a field containing the process ID associated with the job **must** be inserted before the <state> field. Also, more processes created to execute the job may be output on separate lines, using only the process ID and <command> fields. |

### EXIT STATUS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-62-14-001` | Verify that: The following exit values **must** be returned: |

## kill (Built-in)

### DESCRIPTION

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-64-03-001` | Verify that: The kill utility **must** send a signal to the process or processes specified by each pid operand. |
| `SHALL-20-64-03-002` | Verify that: For each pid operand, the kill utility **must** perform actions equivalent to the kill() function defined in the System Interfaces volume of POSIX.1-2024 called with the following arguments: |
| `SHALL-20-64-03-003` | Verify that: The value of the pid operand **must** be used as the pid argument. |

### OPTIONS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-64-04-001` | Verify that: The kill utility **must** conform to XBD 12.2 Utility Syntax Guidelines, [XSI] except that in the last two SYNOPSIS forms, the -signal_number and -signal_name options are usually more than a single character. |
| `SHALL-20-64-04-002` | Verify that: The following options **must** be supported: |
| `SHALL-20-64-04-003` | Verify that: (The letter ell.) Write all values of signal_name supported by the implementation, if no operand is given. If an exit_status operand is given and it is a value of the '?' shell special parameter (see 2.5.2 Special Parameters and wait) corresponding to a process that was terminated or stopped by a signal, the signal_name corresponding to the signal that terminated or stopped the process **must** be written. If an exit_status operand is given and it is the unsigned decimal integer value of a signal number, the signal_name (the symbolic constant name without the SIG prefix defined in the Base Definitions volume of POSIX.1-2024) corresponding to that signal **must** be written. Otherwise, the results are unspecified. |
| `SHALL-20-64-04-004` | Verify that: Specify the signal to send, using one of the symbolic names defined in the <signal.h> header. Values of signal_name **must** be recognized in a case-independent fashion, without the SIG prefix. In addition, the symbolic name 0 **must** be recognized, representing the signal value zero. The corresponding signal **must** be sent instead of SIGTERM. |
| `SHALL-20-64-04-005` | Verify that: If the first argument is a negative integer, it **must** be interpreted as a -signal_number option, not as a negative pid operand specifying a process group. |

### OPERANDS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-64-05-001` | Verify that: The following operands **must** be supported: |
| `SHALL-20-64-05-002` | Verify that: A decimal integer specifying a process or process group to be signaled. The process or processes selected by positive, negative, and zero values of the pid operand **must** be as described for the kill() function. If process number 0 is specified, all processes in the current process group **must** be signaled. For the effects of negative pid numbers, see the kill() function defined in the System Interfaces volume of POSIX.1-2024. If the first pid operand is negative, it should be preceded by "--" to keep it from being interpreted as an option. |

### ENVIRONMENT VARIABLES

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-64-08-001` | Verify that: The following environment variables **must** affect the execution of kill: |

### STDOUT

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-64-10-001` | Verify that: When the -l option is specified, the symbolic name of each signal **must** be written in the following format: |
| `SHALL-20-64-10-002` | Verify that: where the <signal_name> is in uppercase, without the SIG prefix, and the <separator> **must** be either a <newline> or a <space>. For the last signal written, <separator> **must** be a <newline>. |
| `SHALL-20-64-10-003` | Verify that: When both the -l option and exit_status operand are specified, the symbolic name of the corresponding signal **must** be written in the following format: |

### EXIT STATUS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-64-14-001` | Verify that: The following exit values **must** be returned: |

## pwd (Built-in)

### DESCRIPTION

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-99-03-001` | Verify that: The pwd utility **must** write to standard output an absolute pathname of the current working directory, which does not contain the filenames dot or dot-dot. |

### OPTIONS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-99-04-001` | Verify that: The pwd utility **must** conform to XBD 12.2 Utility Syntax Guidelines. |
| `SHALL-20-99-04-002` | Verify that: The following options **must** be supported by the implementation: |
| `SHALL-20-99-04-003` | Verify that: If the PWD environment variable contains an absolute pathname of the current directory and the pathname does not contain any components that are dot or dot-dot, pwd **must** write this pathname to standard output, except that if the PWD environment variable is longer than {PATH_MAX} bytes including the terminating null, it is unspecified whether pwd writes this pathname to standard output or behaves as if the -P option had been specified. Otherwise, the -L option **must** behave as the -P option. |
| `SHALL-20-99-04-004` | Verify that: The pathname written to standard output **must** not contain any components that refer to files of type symbolic link. If there are multiple pathnames that the pwd utility could write to standard output, one beginning with a single <slash> character and one or more beginning with two <slash> characters, then it **must** write the pathname beginning with a single <slash> character. The pathname **must** not contain any unnecessary <slash> characters after the leading one or two <slash> characters. |
| `SHALL-20-99-04-005` | Verify that: If both -L and -P are specified, the last one **must** apply. If neither -L nor -P is specified, the pwd utility **must** behave as if -L had been specified. |

### ENVIRONMENT VARIABLES

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-99-08-001` | Verify that: The following environment variables **must** affect the execution of pwd: |

### EXIT STATUS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-99-14-001` | Verify that: The following exit values **must** be returned: |

### CONSEQUENCES OF ERRORS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-99-15-001` | Verify that: If an error is detected other than a write error when writing to standard output, no output **must** be written to standard output, a diagnostic message **must** be written to standard error, and the exit status **must** be non-zero. |

## read (Built-in)

### DESCRIPTION

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-100-03-001` | Verify that: The read utility **must** read a single logical line from standard input into one or more shell variables. |
| `SHALL-20-100-03-002` | Verify that: If the -r option is not specified, <backslash> **must** act as an escape character. An unescaped <backslash> **must** preserve the literal value of a following <backslash> and **must** prevent a following byte (if any) from being used to split fields, with the exception of either <newline> or the logical line delimiter specified with the -d delim option (if it is used and delim is not <newline>); it is unspecified which. If this excepted character follows the <backslash>, the read utility **must** interpret this as line continuation. The <backslash> and the excepted character **must** be removed before splitting the input into fields. All other unescaped <backslash> characters **must** be removed after splitting the input into fields. |
| `SHALL-20-100-03-003` | Verify that: If standard input is a terminal device and the invoking shell is interactive, read **must** prompt for a continuation line when it reads an input line ending with a <backslash> <newline>, unless the -r option is specified. |
| `SHALL-20-100-03-004` | Verify that: The terminating logical line delimiter (if any) **must** be removed from the input. Then, if the shell variable IFS (see 2.5.3 Shell Variables) is set, and its value is an empty string, the resulting data **must** be assigned to the variable named by the first var operand, and the variables named by other var operands (if any) **must** be set to the empty string. No other processing **must** be performed in this case. |
| `SHALL-20-100-03-005` | Verify that: If IFS is unset, or is set to any non-empty value, then a modified version of the field splitting algorithm specified in 2.6.5 Field Splitting **must** be applied, with the modifications as follows: |
| `SHALL-20-100-03-006` | Verify that: The input to the algorithm **must** be the logical line (minus terminating delimiter) that was read from standard input, and **must** be considered as a single initial field, all of which resulted from expansions, with any escaped byte and the preceding <backslash> escape character treated as if they were the result of a quoted expansion, and all other bytes treated as if they were the results of unquoted expansions. |
| `SHALL-20-100-03-007` | Verify that: The loop over the contents of that initial field **must** cease when either the input is empty or n output fields have been generated, where n is one less than the number of var operands passed to the read utility. Any remaining input in the original field being processed **must** be returned to the read utility "unsplit"; that is, unmodified except that any leading or trailing IFS white space, as defined in 2.6.5 Field Splitting, **must** be removed. |
| `SHALL-20-100-03-008` | Verify that: The specified var operands **must** be processed in the order they appear on the command line, and the output fields generated by the field splitting algorithm **must** be used in the order they were generated, by repeating the following checks until neither is true: |
| `SHALL-20-100-03-009` | Verify that: If more than one var operand is yet to be processed and one or more output fields are yet to be used, the variable named by the first unprocessed var operand **must** be assigned the value of the first unused output field. |
| `SHALL-20-100-03-010` | Verify that: If exactly one var operand is yet to be processed and there was some remaining unsplit input returned from the modified field splitting algorithm, the variable named by the unprocessed var operand **must** be assigned the unsplit input. |
| `SHALL-20-100-03-011` | Verify that: If there are still one or more unprocessed var operands, each of the variables names by those operands **must** be assigned an empty string. |
| `SHALL-20-100-03-012` | Verify that: The setting of variables specified by the var operands **must** affect the current shell execution environment; see 2.13 Shell Execution Environment. An error in setting any variable (such as if a var has previously been marked readonly) **must** be considered an error of read processing, and **must** result in a return value greater than one. Variables named before the one generating the error **must** be set as described above; it is unspecified whether variables named later **must** be set as above, or read simply ceases processing when the error occurs, leaving later named variables unaltered. If read is called in a subshell or separate utility execution environment, such as one of the following: |
| `SHALL-20-100-03-013` | Verify that: it **must** not affect the shell variables in the caller's environment. |
| `SHALL-20-100-03-014` | Verify that: If end-of-file is detected before a terminating logical line delimiter is encountered, the variables specified by the var operands **must** be set as described above and the exit status **must** be 1. |

### OPTIONS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-100-04-001` | Verify that: The read utility **must** conform to XBD 12.2 Utility Syntax Guidelines. |
| `SHALL-20-100-04-002` | Verify that: The following options **must** be supported: |
| `SHALL-20-100-04-003` | Verify that: If delim consists of one single-byte character, that byte **must** be used as the logical line delimiter. If delim is the null string, the logical line delimiter **must** be the null byte. Otherwise, the behavior is unspecified. |

### OPERANDS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-100-05-001` | Verify that: The following operand **must** be supported: |

### ENVIRONMENT VARIABLES

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-100-08-001` | Verify that: The following environment variables **must** affect the execution of read: |
| `SHALL-20-100-08-002` | Verify that: Provide the prompt string that an interactive shell **must** write to standard error when a line ending with a <backslash> <newline> is read and the -r option was not specified. |

### EXIT STATUS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-100-14-001` | Verify that: The following exit values **must** be returned: |

## type (Built-in)

### DESCRIPTION

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-130-03-001` | Verify that: The type utility **must** indicate how each argument would be interpreted if used as a command name. |

### OPERANDS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-130-05-001` | Verify that: The following operand **must** be supported: |

### ENVIRONMENT VARIABLES

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-130-08-001` | Verify that: The following environment variables **must** affect the execution of type: |

### EXIT STATUS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-130-14-001` | Verify that: The following exit values **must** be returned: |

## ulimit (Built-in)

### DESCRIPTION

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-131-03-001` | Verify that: The ulimit utility **must** report or set the resource limits in effect in the process in which it is executed. |
| `SHALL-20-131-03-002` | Verify that: The value unlimited for a resource **must** be considered to be larger than any other limit value. When a resource has this limit value, the implementation **must** not enforce limits on that resource. In locales other than the POSIX locale, ulimit may support additional non-numeric values with the same meaning as unlimited. |
| `SHALL-20-131-03-003` | Verify that: The behavior when resource limits are exceeded **must** be as described in the System Interfaces volume of POSIX.1-2024 for the setrlimit() function. |

### OPTIONS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-131-04-001` | Verify that: The ulimit utility **must** conform to XBD 12.2 Utility Syntax Guidelines, except that: |
| `SHALL-20-131-04-002` | Verify that: Conforming applications **must** specify each option separately; that is, grouping option letters (for example, -fH) need not be recognized by all implementations. |
| `SHALL-20-131-04-003` | Verify that: The following options **must** be supported: |
| `SHALL-20-131-04-004` | Verify that: If the newlimit operand is present, it **must** be used as the new value for both the hard and soft limits. |
| `SHALL-20-131-04-005` | Verify that: If the newlimit operand is not present, -S **must** be the default. |
| `SHALL-20-131-04-006` | Verify that: If no options other than -H or -S are specified, the behavior **must** be as if the -f option was (also) specified. |

### OPERANDS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-131-05-001` | Verify that: The following operand **must** be supported: |
| `SHALL-20-131-05-002` | Verify that: Either an integer value to use as the new limit(s) for the specified resource, in the units specified in OPTIONS, or a non-numeric string indicating no limit, as described in the DESCRIPTION section. Numerals in the range 0 to the maximum limit value supported by the implementation for any resource **must** be syntactically recognized as numeric values. |

### ENVIRONMENT VARIABLES

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-131-08-001` | Verify that: The following environment variables **must** affect the execution of ulimit: |

### STDOUT

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-131-10-001` | Verify that: If the -a option is specified, the output written for each resource **must** consist of one line that includes: |
| `SHALL-20-131-10-002` | Verify that: The format used within each line is unspecified, except that the format used for the limit value **must** be as described below for the case where a single limit value is written. |
| `SHALL-20-131-10-003` | Verify that: If the resource being reported has a numeric limit, the limit value **must** be written in the following format: |
| `SHALL-20-131-10-004` | Verify that: If the resource being reported does not have a numeric limit, in the POSIX locale the following format **must** be used: |

### EXIT STATUS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-131-14-001` | Verify that: The following exit values **must** be returned: |

## umask (Built-in)

### DESCRIPTION

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-132-03-001` | Verify that: The umask utility **must** set the file mode creation mask of the current shell execution environment (see 2.13 Shell Execution Environment) to the value specified by the mask operand. This mask **must** affect the initial value of the file permission bits of subsequently created files. If umask is called in a subshell or separate utility execution environment, such as one of the following: |
| `SHALL-20-132-03-002` | Verify that: it **must** not affect the file mode creation mask of the caller's environment. |
| `SHALL-20-132-03-003` | Verify that: If the mask operand is not specified, the umask utility **must** write to standard output the value of the file mode creation mask of the invoking process. |

### OPTIONS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-132-04-001` | Verify that: The umask utility **must** conform to XBD 12.2 Utility Syntax Guidelines. |
| `SHALL-20-132-04-002` | Verify that: The following option **must** be supported: |
| `SHALL-20-132-04-003` | Verify that: The default output style is unspecified, but **must** be recognized on a subsequent invocation of umask on the same system as a mask operand to restore the previous file mode creation mask. |

### OPERANDS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-132-05-001` | Verify that: The following operand **must** be supported: |
| `SHALL-20-132-05-002` | Verify that: For a symbolic_mode value, the new value of the file mode creation mask **must** be the logical complement of the file permission bits portion of the file mode specified by the symbolic_mode string. |
| `SHALL-20-132-05-003` | Verify that: In a symbolic_mode value, the permissions op characters '+' and '-' **must** be interpreted relative to the current file mode creation mask; '+' **must** cause the bits for the indicated permissions to be cleared in the mask; '-' **must** cause the bits for the indicated permissions to be set in the mask. |
| `SHALL-20-132-05-004` | Verify that: The file mode creation mask **must** be set to the resulting numeric value. |
| `SHALL-20-132-05-005` | Verify that: The default output of a prior invocation of umask on the same system with no operand also **must** be recognized as a mask operand. |

### ENVIRONMENT VARIABLES

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-132-08-001` | Verify that: The following environment variables **must** affect the execution of umask: |

### STDOUT

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-132-10-001` | Verify that: When the mask operand is not specified, the umask utility **must** write a message to standard output that can later be used as a umask mask operand. |
| `SHALL-20-132-10-002` | Verify that: If -S is specified, the message **must** be in the following format: |
| `SHALL-20-132-10-003` | Verify that: where the three values **must** be combinations of letters from the set {r, w, x}; the presence of a letter **must** indicate that the corresponding bit is clear in the file mode creation mask. |
| `SHALL-20-132-10-004` | Verify that: If a mask operand is specified, there **must** be no output written to standard output. |

### EXIT STATUS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-132-14-001` | Verify that: The following exit values **must** be returned: |

## unalias (Built-in)

### DESCRIPTION

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-133-03-001` | Verify that: The unalias utility **must** remove the definition for each alias name specified. See 2.3.1 Alias Substitution. The aliases **must** be removed from the current shell execution environment; see 2.13 Shell Execution Environment. |

### OPTIONS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-133-04-001` | Verify that: The unalias utility **must** conform to XBD 12.2 Utility Syntax Guidelines. |
| `SHALL-20-133-04-002` | Verify that: The following option **must** be supported: |

### OPERANDS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-133-05-001` | Verify that: The following operand **must** be supported: |

### ENVIRONMENT VARIABLES

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-133-08-001` | Verify that: The following environment variables **must** affect the execution of unalias: |

### EXIT STATUS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-133-14-001` | Verify that: The following exit values **must** be returned: |

## wait (Built-in)

### DESCRIPTION

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-147-03-001` | Verify that: The wait utility **must** wait for one or more child processes whose process IDs are known in the current shell execution environment (see 2.13 Shell Execution Environment) to terminate. |
| `SHALL-20-147-03-002` | Verify that: If the wait utility is invoked with no operands, it **must** wait until all process IDs known to the invoking shell have terminated and exit with a zero exit status. |
| `SHALL-20-147-03-003` | Verify that: If one or more pid operands are specified that represent known process IDs, the wait utility **must** wait until all of them have terminated. If one or more pid operands are specified that represent unknown process IDs, wait **must** treat them as if they were known process IDs that exited with exit status 127. The exit status returned by the wait utility **must** be the exit status of the process requested by the last pid operand. |
| `SHALL-20-147-03-004` | Verify that: Once a process ID that is known in the current shell execution environment (see 2.13 Shell Execution Environment) has been successfully waited for, it **must** be removed from the list of process IDs that are known in the current shell execution environment. If the process ID is associated with a background job, the corresponding job **must** also be removed from the list of background jobs. |

### OPERANDS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-147-05-001` | Verify that: The following operand **must** be supported: |
| `SHALL-20-147-05-002` | Verify that: A job ID (see XBD 3.182 Job ID) that identifies a process group in the case of a job-control background job, or a process ID in the case of a non-job-control background job (if supported), to be waited for. The job ID notation is applicable only for invocations of wait in the current shell execution environment; see 2.13 Shell Execution Environment. The exit status of wait **must** be determined by the exit status of the last pipeline to be executed. |

### ENVIRONMENT VARIABLES

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-147-08-001` | Verify that: The following environment variables **must** affect the execution of wait: |

### EXIT STATUS

| Test ID | Behavior to Verify |
|---------|--------------------|
| `SHALL-20-147-14-001` | Verify that: If one or more operands were specified, all of them have terminated or were not known in the invoking shell execution environment, and the status of the last operand specified is known, then the exit status of wait **must** be the status of the last operand specified. If the process terminated abnormally due to the receipt of a signal, the exit status **must** be greater than 128 and **must** be distinct from the exit status generated by other signals, but the exact value is unspecified. (See the kill -l option.) Otherwise, the wait utility **must** exit with one of the following values: |

