# Test Suite for Utility: sh

This test suite covers the **sh** utility as specified by
POSIX.1-2024. The `sh` utility is a command language interpreter
that executes commands read from a command line string, standard
input, or a specified file.

## Table of contents

- [utility: sh](#utility-sh)

## utility: sh

#### NAME

> sh — shell, the standard command language interpreter

#### SYNOPSIS

> ```
> [OB] sh [-abCefhimnuvx] [-o option]...
> [+abCefhimnuvx]
> [+o option]...
> [command_file [argument...]]
> [OB] sh -c [-abCefhimnuvx] [-o option]...
> [+abCefhimnuvx]
> [+o option]...
> command_string [command_name
> [argument...]]
> [OB] sh -s [-abCefhimnuvx] [-o option]...
> [+abCefhimnuvx]
> [+o option]...
> [argument...]
> ```

#### DESCRIPTION

> The *sh* utility is a command language interpreter that shall execute commands read from a command line string, the standard input, or a specified file. The application shall ensure that the commands to be executed are expressed in the language described in [*2. Shell Command Language*](docs/posix/md/utilities/V3_chap02.md#2-shell-command-language).
>
> Pathname expansion shall not fail due to the size of a file.
>
> Shell input and output redirections have an implementation-defined offset maximum that is established in the open file description.

#### OPTIONS

> The *sh* utility shall conform to XBD [*12.2 Utility Syntax Guidelines*](docs/posix/md/basedefs/V1_chap12.md#122-utility-syntax-guidelines), with an extension for support of a leading `<plus-sign>` (`'+'`) as noted below.
>
> The **-a**, **-b**, **-C**, **-e**, **-f**, **-h**, **-m**, **-n**, **-o** *option*, **-u**, **-v**, and **-x** options are described as part of the [*set*](docs/posix/md/utilities/V3_chap02.md#set) utility in [*2.15 Special Built-In Utilities*](docs/posix/md/utilities/V3_chap02.md#215-special-built-in-utilities). The option letters derived from the [*set*](docs/posix/md/utilities/V3_chap02.md#set) special built-in shall also be accepted with a leading `<plus-sign>` (`'+'`) instead of a leading `<hyphen-minus>` (meaning the reverse case of the option as described in this volume of POSIX.1-2024). If the **-o** or **+o** option is specified without an option-argument, the behavior is unspecified.
>
> The following additional options shall be supported:
>
> - **-c**: Read commands from the *command_string* operand. Set the value of special parameter 0 (see [*2.5.2 Special Parameters*](docs/posix/md/utilities/V3_chap02.md#252-special-parameters)) from the value of the *command_name* operand and the positional parameters ($1, $2, and so on) in sequence from the remaining *argument* operands. No commands shall be read from the standard input.
> - **-i**: Specify that the shell is *interactive*; see below. An implementation may treat specifying the **-i** option as an error if the real user ID of the calling process does not equal the effective user ID or if the real group ID does not equal the effective group ID.
> - **-s**: Read commands from the standard input.
>
> If there are no operands and the **-c** option is not specified, the **-s** option shall be assumed.
>
> If the **-i** option is present, or if the shell reads commands from the standard input and the shell's standard input and standard error are attached to a terminal, the shell is considered to be *interactive*.

#### OPERANDS

> The following operands shall be supported:
>
> - `-`: A single `<hyphen-minus>` shall be treated as the first operand and then ignored. If both `'-'` and `"--"` are given as arguments, or if other operands precede the single `<hyphen-minus>`, the results are undefined.
> - *argument*: The positional parameters ($1, $2, and so on) shall be set to *arguments*, if any.
> - *command_file*: The pathname of a file containing commands. If the pathname contains one or more `<slash>` characters, the implementation attempts to read that file; the file need not be executable. If the pathname does not contain a `<slash>` character:
>
>     - The implementation shall attempt to read that file from the current working directory; the file need not be executable.
>     - If the file is not in the current working directory, the implementation may perform a search for an executable file using the value of *PATH ,* as described in [*2.9.1.4 Command Search and Execution*](docs/posix/md/utilities/V3_chap02.md#2914-command-search-and-execution).
>
>   Special parameter 0 (see [*2.5.2 Special Parameters*](docs/posix/md/utilities/V3_chap02.md#252-special-parameters)) shall be set to the value of *command_file*. If *sh* is called using a synopsis form that omits *command_file*, special parameter 0 shall be set to the value of the first argument passed to *sh* from its parent (for example, *argv*[0] for a C program), which is normally a pathname used to execute the *sh* utility.
> - *command_name*: A string assigned to special parameter 0 when executing the commands in *command_string* . If *command_name* is not specified, special parameter 0 shall be set to the value of the first argument passed to *sh* from its parent (for example, *argv* [0] for a C program), which is normally a pathname used to execute the *sh* utility.
> - *command_string*: A string that shall be interpreted by the shell as one or more commands, as if the string were the argument to the [*system*()](docs/posix/md/functions/system.md) function defined in the System Interfaces volume of POSIX.1-2024. If the *command_string* operand is an empty string, *sh* shall exit with a zero exit status.

#### STDIN

> The standard input shall be used only if one of the following is true:
>
> - The **-s** option is specified.
> - The **-c** option is not specified and no operands are specified.
> - The script executes one or more commands that require input from standard input (such as a [*read*](docs/posix/md/utilities/read.md) command that does not redirect its input).
>
> See the INPUT FILES section.
>
> When the shell is using standard input and it invokes a command that also uses standard input, the shell shall ensure that the standard input file pointer points directly after the command it has read when the command begins execution. It shall not read ahead in such a manner that any characters intended to be read by the invoked command are consumed by the shell (whether interpreted by the shell or not) or that characters that are not read by the invoked command are not seen by the shell. When the command expecting to read standard input is started asynchronously by an interactive shell, it is unspecified whether characters are read by the command or interpreted by the shell.
>
> If the standard input to *sh* is a FIFO or terminal device and is set to non-blocking reads, then *sh* shall enable blocking reads on standard input. This shall remain in effect when the command completes.

#### INPUT FILES

> The input file can be of any type, but the initial portion of the file intended to be parsed according to the shell grammar (see [*2.10.2 Shell Grammar Rules*](docs/posix/md/utilities/V3_chap02.md#2102-shell-grammar-rules)) shall consist of characters and shall not contain the NUL character. The shell shall not enforce any line length limits. If the input file consists solely of zero or more blank lines and comments, *sh* shall exit with a zero exit status.

#### ENVIRONMENT VARIABLES

> The following environment variables shall affect the execution of *sh*:
>
> - *ENV*: This variable, when and only when an interactive shell is invoked, shall be subjected to parameter expansion (see [*2.6.2 Parameter Expansion*](docs/posix/md/utilities/V3_chap02.md#262-parameter-expansion)) by the shell, and the resulting value shall be used as a pathname of a file containing shell commands to execute in the current environment. The file need not be executable. If the expanded value of *ENV* is not an absolute pathname, the results are unspecified. *ENV* shall be ignored if the real and effective user IDs or real and effective group IDs of the process are different. The file specified by *ENV* need not be processed if the file can be written by any user other than the user identified by the real (and effective) user ID of the shell process.
> - *FCEDIT*: This variable, when expanded by the shell, shall determine the default value for the **-e** *editor* option's *editor* option-argument. If *FCEDIT* is null or unset, [*ed*](docs/posix/md/utilities/ed.md) shall be used as the editor.
> - *HISTFILE*: Determine a pathname naming a command history file. If the *HISTFILE* variable is not set, the shell may attempt to access or create a file **.sh_history** in the directory referred to by the *HOME* environment variable. If the shell cannot obtain both read and write access to, or create, the history file, it shall use an unspecified mechanism that allows the history to operate properly. (References to history "file" in this section shall be understood to mean this unspecified mechanism in such cases.) An implementation may choose to access this variable only when initializing the history file; this initialization shall occur when [*fc*](docs/posix/md/utilities/fc.md) or *sh* first attempt to retrieve entries from, or add entries to, the file, as the result of commands issued by the user, the file named by the *ENV* variable, or implementation-defined system start-up files. Implementations may choose to disable the history list mechanism for users with appropriate privileges who do not set *HISTFILE ;* the specific circumstances under which this occurs are implementation-defined. If more than one instance of the shell is using the same history file, it is unspecified how updates to the history file from those shells interact. As entries are deleted from the history file, they shall be deleted oldest first. It is unspecified when history file entries are physically removed from the history file.
> - *HISTSIZE*: Determine a decimal number representing the limit to the number of previous commands that are accessible. If this variable is unset, an unspecified default greater than or equal to 128 shall be used. The maximum number of commands in the history list is unspecified, but shall be at least 128. An implementation may choose to access this variable only when initializing the history file, as described under *HISTFILE .* Therefore, it is unspecified whether changes made to *HISTSIZE* after the history file has been initialized are effective.
> - *HOME*: Determine the pathname of the user's home directory. The contents of *HOME* are used in tilde expansion as described in [*2.6.1 Tilde Expansion*](docs/posix/md/utilities/V3_chap02.md#261-tilde-expansion).
> - *LANG*: Provide a default value for the internationalization variables that are unset or null. (See XBD [*8.2 Internationalization Variables*](docs/posix/md/basedefs/V1_chap08.md#82-internationalization-variables) for the precedence of internationalization variables used to determine the values of locale categories.)
> - *LC_ALL*: If set to a non-empty string value, override the values of all the other internationalization variables.
> - *LC_COLLATE*: Determine the behavior of range expressions, equivalence classes, and multi-character collating elements within pattern matching.
> - *LC_CTYPE*: Determine the locale for the interpretation of sequences of bytes of text data as characters (for example, single-byte as opposed to multi-byte characters in arguments and input files), which characters are defined as letters (character class **alpha**), and the behavior of character classes within pattern matching.
> - *LC_MESSAGES*: Determine the locale that should be used to affect the format and contents of diagnostic messages written to standard error.
> - *MAIL*: Determine a pathname of the user's mailbox file for purposes of incoming mail notification. If this variable is set, the shell shall inform the user if the file named by the variable is created or if its modification time has changed. Informing the user shall be accomplished by writing a string of unspecified format to standard error prior to the writing of the next primary prompt string. Such check shall be performed only after the completion of the interval defined by the *MAILCHECK* variable after the last such check. The user shall be informed only if *MAIL* is set and *MAILPATH* is not set.
> - *MAILCHECK*: Establish a decimal integer value that specifies how often (in seconds) the shell shall check for the arrival of mail in the files specified by the *MAILPATH* or *MAIL* variables. The default value shall be 600 seconds. If set to zero, the shell shall check before issuing each primary prompt.
> - *MAILPATH*: Provide a list of pathnames and optional messages separated by `<colon>` characters. If this variable is set, the shell shall inform the user if any of the files named by the variable are created or if any of their modification times change. (See the preceding entry for *MAIL* for descriptions of mail arrival and user informing.) Each pathname can be followed by `'%'` and a string that shall be subjected to parameter expansion and written to standard error when the modification time changes. If a `'%'` character in the pathname is preceded by a `<backslash>`, it shall be treated as a literal `'%'` in the pathname. The default message is unspecified. The *MAILPATH* environment variable takes precedence over the *MAIL* variable.
> - *NLSPATH*: Determine the location of messages objects and message catalogs.
> - *PATH*: Establish a string formatted as described in XBD [*8. Environment Variables*](docs/posix/md/basedefs/V1_chap08.md#8-environment-variables), used to effect command interpretation; see [*2.9.1.4 Command Search and Execution*](docs/posix/md/utilities/V3_chap02.md#2914-command-search-and-execution).
> - *PWD*: This variable shall represent an absolute pathname of the current working directory. Assignments to this variable may be ignored.

#### ASYNCHRONOUS EVENTS

> The *sh* utility shall take the standard action for all signals (see [*1.4 Utility Description Defaults*](docs/posix/md/utilities/V3_chap01.md#14-utility-description-defaults)) with the following exceptions.
>
> If the shell is interactive, SIGINT signals received during command line editing shall be handled as described in the EXTENDED DESCRIPTION, and SIGINT signals received at other times shall be caught but no action performed.
>
> If the shell is interactive:
>
> - SIGQUIT and SIGTERM signals shall be ignored.
> - If the **-m** option is in effect, SIGTTIN, SIGTTOU, and SIGTSTP signals shall be ignored.
> - If the **-m** option is not in effect, it is unspecified whether SIGTTIN, SIGTTOU, and SIGTSTP signals are ignored, set to the default action, or caught. If they are caught, the shell shall, in the signal-catching function, set the signal to the default action and raise the signal (after taking any appropriate steps, such as restoring terminal settings).
>
> The standard actions, and the actions described above for interactive shells, can be overridden by use of the [*trap*](docs/posix/md/utilities/trap.md) special built-in utility (see [*trap*](docs/posix/md/utilities/V3_chap02.md#tag_19_29) and [*2.12 Signals and Error Handling*](docs/posix/md/utilities/V3_chap02.md#212-signals-and-error-handling)).

#### STDOUT

> See the STDERR section.

#### STDERR

> Except as otherwise stated (by the descriptions of any invoked utilities or in interactive mode), standard error shall be used only for diagnostic messages.

#### OUTPUT FILES

> None.

#### EXTENDED DESCRIPTION

> See [*2. Shell Command Language*](docs/posix/md/utilities/V3_chap02.md#2-shell-command-language). The functionality described in the rest of the EXTENDED DESCRIPTION section shall be provided on implementations that support the User Portability Utilities option (and the rest of this section is not further shaded for this option).
>
> ##### Command History List
>
> When the *sh* utility is being used interactively, it shall maintain a list of commands previously entered from the terminal in the file named by the *HISTFILE* environment variable. The type, size, and internal format of this file are unspecified. Multiple *sh* processes can share access to the file for a user, if file access permissions allow this; see the description of the *HISTFILE* environment variable.
>
> ##### Command Line Editing
>
> When *sh* is being used interactively from a terminal, the current command and the command history (see [*fc*](docs/posix/md/utilities/fc.md)) can be edited using [*vi*](docs/posix/md/utilities/vi.md)-mode command line editing. This mode uses commands, described below, similar to a subset of those described in the [*vi*](docs/posix/md/utilities/vi.md) utility. Implementations may offer other command line editing modes corresponding to other editing utilities.
>
> The command [*set*](docs/posix/md/utilities/V3_chap02.md#set) **-o** [*vi*](docs/posix/md/utilities/vi.md) shall enable [*vi*](docs/posix/md/utilities/vi.md)-mode editing and place *sh* into [*vi*](docs/posix/md/utilities/vi.md) insert mode (see [Command Line Editing (vi-mode)](#command-line-editing-vi-mode)). This command also shall disable any other editing mode that the implementation may provide. The command [*set*](docs/posix/md/utilities/V3_chap02.md#set) **+o** [*vi*](docs/posix/md/utilities/vi.md) disables [*vi*](docs/posix/md/utilities/vi.md)-mode editing.
>
> Certain block-mode terminals may be unable to support shell command line editing. If a terminal is unable to provide either edit mode, it need not be possible to [*set*](docs/posix/md/utilities/V3_chap02.md#set) **-o** [*vi*](docs/posix/md/utilities/vi.md) when using the shell on this terminal.
>
> In the following sections, the characters *erase*, *interrupt*, *kill*, and *end-of-file* are those set by the [*stty*](docs/posix/md/utilities/stty.md) utility.
>
> ##### Command Line Editing (vi-mode)
>
> In [*vi*](docs/posix/md/utilities/vi.md) editing mode, there shall be a distinguished line, the edit line. All the editing operations which modify a line affect the edit line. The edit line is always the newest line in the command history buffer.
>
> With [*vi*](docs/posix/md/utilities/vi.md)-mode enabled, *sh* can be switched between insert mode and command mode.
>
> When in insert mode, an entered character shall be inserted into the command line, except as noted in [vi Line Editing Insert Mode](#vi-line-editing-insert-mode). Upon entering *sh* and after termination of the previous command, *sh* shall be in insert mode.
>
> Typing an escape character shall switch *sh* into command mode (see [vi Line Editing Command Mode](#vi-line-editing-command-mode)). In command mode, an entered character shall either invoke a defined operation, be used as part of a multi-character operation, or be treated as an error. A character that is not recognized as part of an editing command shall terminate any specific editing command and shall alert the terminal. If *sh* receives a SIGINT signal in command mode (whether generated by typing the *interrupt* character or by other means), it shall terminate command line editing on the current command line, reissue the prompt on the next line of the terminal, and reset the command history (see [*fc*](docs/posix/md/utilities/fc.md)) so that the most recently executed command is the previous command (that is, the command that was being edited when it was interrupted is not re-entered into the history).
>
> In the following sections, the phrase "move the cursor to the beginning of the word" shall mean "move the cursor to the first character of the current word" and the phrase "move the cursor to the end of the word" shall mean "move the cursor to the last character of the current word". The phrase "beginning of the command line" indicates the point between the end of the prompt string issued by the shell (or the beginning of the terminal line, if there is no prompt string) and the first character of the command text.
>
> ##### vi Line Editing Insert Mode
>
> While in insert mode, any character typed shall be inserted in the current command line, unless it is from the following set.
>
> - `<newline>`: Execute the current command line. If the current command line is not empty, this line shall be entered into the command history (see [*fc*](docs/posix/md/utilities/fc.md)).
> - *erase*: Delete the character previous to the current cursor position and move the current cursor position back one character. In insert mode, characters shall be erased from both the screen and the buffer when backspacing.
> - *interrupt*: If *sh* receives a SIGINT signal in insert mode (whether generated by typing the *interrupt* character or by other means), it shall terminate command line editing with the same effects as described for interrupting command mode; see [Command Line Editing (vi-mode)](#command-line-editing-vi-mode).
> - *kill*: Clear all the characters from the input line.
> - `<control>`-V: Insert the next character input, even if the character is otherwise a special insert mode character.
> - `<control>`-W: Delete the characters from the one preceding the cursor to the preceding word boundary. The word boundary in this case is the closer to the cursor of either the beginning of the line or a character that is in neither the **blank** nor **punct** character classification of the current locale.
> - *end-of-file*: Interpreted as the end of input in *sh*. This interpretation shall occur only at the beginning of an input line. If *end-of-file* is entered other than at the beginning of the line, the results are unspecified.
> - `<ESC>`: Place *sh* into command mode.
>
> ##### vi Line Editing Command Mode
>
> In command mode for the command line editing feature, decimal digits not beginning with 0 that precede a command letter shall be remembered. Some commands use these decimal digits as a count number that affects the operation.
>
> The term *motion command* represents one of the commands:
>
> ```
> <space>  0  b  F  l  W  ^  $  ;  E  f  T  w  |  ,  B  e  h  t
> ```
>
> If the current line is not the edit line, any command that modifies the current line shall cause the content of the current line to replace the content of the edit line, and the current line shall become the edit line. This replacement cannot be undone (see the **u** and **U** commands below). The modification requested shall then be performed to the edit line. When the current line is the edit line, the modification shall be done directly to the edit line.
>
> Any command that is preceded by *count* shall take a count (the numeric value of any preceding decimal digits). Unless otherwise noted, this count shall cause the specified operation to repeat by the number of times specified by the count. Also unless otherwise noted, a *count* that is out of range is considered an error condition and shall alert the terminal, but neither the cursor position, nor the command line, shall change.
>
> The terms *word* and *bigword* are used as defined in the [*vi*](docs/posix/md/utilities/vi.md) description. The term *save buffer* corresponds to the term *unnamed buffer* in [*vi*](docs/posix/md/utilities/vi.md).
>
> The following commands shall be recognized in command mode:
>
> - `<newline>`: Execute the current command line. If the current command line is not empty, this line shall be entered into the command history (see [*fc*](docs/posix/md/utilities/fc.md)).
> - `<control>`-L: Redraw the current command line. Position the cursor at the same location on the redrawn line.
> - **#**: Insert the character `'#'` at the beginning of the current command line and treat the resulting edit line as a comment. This line shall be entered into the command history; see [*fc*](docs/posix/md/utilities/fc.md).
> - **=**: Display the possible shell word expansions (see [*2.6 Word Expansions*](docs/posix/md/utilities/V3_chap02.md#26-word-expansions) ) of the bigword at the current command line position.
>
>     - **Note:** This does not modify the content of the current line, and therefore does not cause the current line to become the edit line.
>
>   These expansions shall be displayed on subsequent terminal lines. If the bigword contains none of the characters `'?'`, `'*'`, or `'['`, an `<asterisk>` (`'*'`) shall be implicitly assumed at the end. If any directories are matched, these expansions shall have a `'/'` character appended. After the expansion, the line shall be redrawn, the cursor repositioned at the current cursor position, and *sh* shall be placed in command mode.
> - **\**: Perform pathname expansion (see [*2.6.6 Pathname Expansion*](docs/posix/md/utilities/V3_chap02.md#266-pathname-expansion)) on the current bigword, up to the largest set of characters that can be matched uniquely. If the bigword contains none of the characters `'?'`, `'*'`, or `'['`, an `<asterisk>` (`'*'`) shall be implicitly assumed at the end. This maximal expansion then shall replace the original bigword in the command line, and the cursor shall be placed after this expansion. If the resulting bigword completely and uniquely matches a directory, a `'/'` character shall be inserted directly after the bigword. If some other file is completely matched, a single `<space>` shall be inserted after the bigword. After this operation, *sh* shall be placed in insert mode.
> - *****: Perform pathname expansion on the current bigword and insert all expansions into the command to replace the current bigword, with each expansion separated by a single `<space>`. If at the end of the line, the current cursor position shall be moved to the first column position following the expansions and *sh* shall be placed in insert mode. Otherwise, the current cursor position shall be the last column position of the first character after the expansions and *sh* shall be placed in insert mode. If the current bigword contains none of the characters `'?'`, `'*'`, or `'['`, before the operation, an `<asterisk>` (`'*'`) shall be implicitly assumed at the end.
> - **@***letter*: Insert the value of the alias named *_letter*. The symbol *letter* represents a single alphabetic character from the portable character set; implementations may support additional characters as an extension. If the alias *_letter* contains other editing commands, these commands shall be performed as part of the insertion. If no alias *_letter* is enabled, this command shall have no effect.
> - **[***count***]~**: Convert, if the current character is a lowercase letter, to the equivalent uppercase letter and *vice versa*, as prescribed by the current locale. The current cursor position then shall be advanced by one character. If the cursor was positioned on the last character of the line, the case conversion shall occur, but the cursor shall not advance. If the `'~'` command is preceded by a *count*, that number of characters shall be converted, and the cursor shall be advanced to the character position after the last character converted. If the *count* is larger than the number of characters after the cursor, this shall not be considered an error; the cursor shall advance to the last character on the line.
> - **[***count***].**: Repeat the most recent non-motion command, even if it was executed on an earlier command line. If the previous command was preceded by a *count*, and no count is given on the `'.'` command, the count from the previous command shall be included as part of the repeated command. If the `'.'` command is preceded by a *count*, this shall override any *count* argument to the previous command. The *count* specified in the `'.'` command shall become the count for subsequent `'.'` commands issued without a count.
> - **[***number***]v**: Invoke the [*vi*](docs/posix/md/utilities/vi.md) editor to edit the current command line in a temporary file. When the editor exits, the commands in the temporary file shall be executed and placed in the command history. If a *number* is included, it specifies the command number in the command history to be edited, rather than the current command line.
> - **[***count***]l** (ell)
> - **[***count***]**`<space>`: Move the current cursor position to the next character position. If the cursor was positioned on the last character of the line, the terminal shall be alerted and the cursor shall not be advanced. If the *count* is larger than the number of characters after the cursor, this shall not be considered an error; the cursor shall advance to the last character on the line.
> - **[***count***]h**: Move the current cursor position to the *count*th (default 1) previous character position. If the cursor was positioned on the first character of the line, the terminal shall be alerted and the cursor shall not be moved. If the count is larger than the number of characters before the cursor, this shall not be considered an error; the cursor shall move to the first character on the line.
> - **[***count***]w**: Move to the start of the next word. If the cursor was positioned on the last character of the line, the terminal shall be alerted and the cursor shall not be advanced. If the *count* is larger than the number of words after the cursor, this shall not be considered an error; the cursor shall advance to the last character on the line.
> - **[***count***]W**: Move to the start of the next bigword. If the cursor was positioned on the last character of the line, the terminal shall be alerted and the cursor shall not be advanced. If the *count* is larger than the number of bigwords after the cursor, this shall not be considered an error; the cursor shall advance to the last character on the line.
> - **[***count***]e**: Move to the end of the current word. If at the end of a word, move to the end of the next word. If the cursor was positioned on the last character of the line, the terminal shall be alerted and the cursor shall not be advanced. If the *count* is larger than the number of words after the cursor, this shall not be considered an error; the cursor shall advance to the last character on the line.
> - **[***count***]E**: Move to the end of the current bigword. If at the end of a bigword, move to the end of the next bigword. If the cursor was positioned on the last character of the line, the terminal shall be alerted and the cursor shall not be advanced. If the *count* is larger than the number of bigwords after the cursor, this shall not be considered an error; the cursor shall advance to the last character on the line.
> - **[***count***]b**: Move to the beginning of the current word. If at the beginning of a word, move to the beginning of the previous word. If the cursor was positioned on the first character of the line, the terminal shall be alerted and the cursor shall not be moved. If the *count* is larger than the number of words preceding the cursor, this shall not be considered an error; the cursor shall return to the first character on the line.
> - **[***count***]B**: Move to the beginning of the current bigword. If at the beginning of a bigword, move to the beginning of the previous bigword. If the cursor was positioned on the first character of the line, the terminal shall be alerted and the cursor shall not be moved. If the *count* is larger than the number of bigwords preceding the cursor, this shall not be considered an error; the cursor shall return to the first character on the line.
> - **^**: Move the current cursor position to the first character on the input line that is not a `<blank>`.
> - **$**: Move to the last character position on the current command line.
> - **0**: (Zero.) Move to the first character position on the current command line.
> - **[***count***]|**: Move to the *count*th character position on the current command line. If no number is specified, move to the first position. The first character position shall be numbered 1. If the count is larger than the number of characters on the line, this shall not be considered an error; the cursor shall be placed on the last character on the line.
> - **[***count***]f***c*: Move to the first occurrence of the character `'c'` that occurs after the current cursor position. If the cursor was positioned on the last character of the line, the terminal shall be alerted and the cursor shall not be advanced. If the character `'c'` does not occur in the line after the current cursor position, the terminal shall be alerted and the cursor shall not be moved.
> - **[***count***]F***c*: Move to the first occurrence of the character `'c'` that occurs before the current cursor position. If the cursor was positioned on the first character of the line, the terminal shall be alerted and the cursor shall not be moved. If the character `'c'` does not occur in the line before the current cursor position, the terminal shall be alerted and the cursor shall not be moved.
> - **[***count***]t***c*: Move to the character before the first occurrence of the character `'c'` that occurs after the current cursor position. If the cursor was positioned on the last character of the line, the terminal shall be alerted and the cursor shall not be advanced. If the character `'c'` does not occur in the line after the current cursor position, the terminal shall be alerted and the cursor shall not be moved.
> - **[***count***]T***c*: Move to the character after the first occurrence of the character `'c'` that occurs before the current cursor position. If the cursor was positioned on the first character of the line, the terminal shall be alerted and the cursor shall not be moved. If the character `'c'` does not occur in the line before the current cursor position, the terminal shall be alerted and the cursor shall not be moved.
> - **[***count***];**: Repeat the most recent **f**, **F**, **t**, or **T** command. Any number argument on that previous command shall be ignored. Errors are those described for the repeated command.
> - **[***count***],**: Repeat the most recent **f**, **F**, **t**, or **T** command. Any number argument on that previous command shall be ignored. However, reverse the direction of that command.
> - **a**: Enter insert mode after the current cursor position. Characters that are entered shall be inserted before the next character.
> - **A**: Enter insert mode after the end of the current command line.
> - **i**: Enter insert mode at the current cursor position. Characters that are entered shall be inserted before the current character.
> - **I**: Enter insert mode at the beginning of the current command line.
> - **R**: Enter insert mode, replacing characters from the command line beginning at the current cursor position.
> - **[***count***]c***motion*: Delete the characters between the current cursor position and the cursor position that would result from the specified motion command. Then enter insert mode before the first character following any deleted characters. If *count* is specified, it shall be applied to the motion command. A *count* shall be ignored for the following motion commands:
>
>   ```
>   0    ^    $    c
>   ```
>
>   If the motion command is the character `'c'`, the current command line shall be cleared and insert mode shall be entered. If the motion command would move the current cursor position toward the beginning of the command line, the character under the current cursor position shall not be deleted. If the motion command would move the current cursor position toward the end of the command line, the character under the current cursor position shall be deleted. If the *count* is larger than the number of characters between the current cursor position and the end of the command line toward which the motion command would move the cursor, this shall not be considered an error; all of the remaining characters in the aforementioned range shall be deleted and insert mode shall be entered. If the motion command is invalid, the terminal shall be alerted, the cursor shall not be moved, and no text shall be deleted.
> - **C**: Delete from the current character to the end of the line and enter insert mode at the new end-of-line.
> - **S**: Clear the entire edit line and enter insert mode.
> - **[***count***]r***c*: Replace the current character with the character `'c'`. With a number *count*, replace the current and the following *count*-1 characters. After this command, the current cursor position shall be on the last character that was changed. If the *count* is larger than the number of characters after the cursor, this shall not be considered an error; all of the remaining characters shall be changed.
> - **[***count***]_**: Append a `<space>` after the current character position and then append the last bigword in the previous input line after the `<space>`. Then enter insert mode after the last character just appended. With a number *count*, append the *count*th bigword in the previous line.
> - **[***count***]x**: Delete the character at the current cursor position and place the deleted characters in the save buffer. If the cursor was positioned on the last character of the line, the character shall be deleted and the cursor position shall be moved to the previous character (the new last character). If the *count* is larger than the number of characters after the cursor, this shall not be considered an error; all the characters from the cursor to the end of the line shall be deleted.
> - **[***count***]X**: Delete the character before the current cursor position and place the deleted characters in the save buffer. The character under the current cursor position shall not change. If the cursor was positioned on the first character of the line, the terminal shall be alerted, and the **X** command shall have no effect. If the line contained a single character, the **X** command shall have no effect. If the line contained no characters, the terminal shall be alerted and the cursor shall not be moved. If the *count* is larger than the number of characters before the cursor, this shall not be considered an error; all the characters from before the cursor to the beginning of the line shall be deleted.
> - **[***count***]d***motion*: Delete the characters between the current cursor position and the character position that would result from the motion command. A number *count* repeats the motion command *count* times. If the motion command would move toward the beginning of the command line, the character under the current cursor position shall not be deleted. If the motion command is **d** , the entire current command line shall be cleared. If the *count* is larger than the number of characters between the current cursor position and the end of the command line toward which the motion command would move the cursor, this shall not be considered an error; all of the remaining characters in the aforementioned range shall be deleted. The deleted characters shall be placed in the save buffer.
> - **D**: Delete all characters from the current cursor position to the end of the line. The deleted characters shall be placed in the save buffer.
> - **[***count***]y***motion*: Yank (that is, copy) the characters from the current cursor position to the position resulting from the motion command into the save buffer. A number *count* shall be applied to the motion command. If the motion command would move toward the beginning of the command line, the character under the current cursor position shall not be included in the set of yanked characters. If the motion command is **y** , the entire current command line shall be yanked into the save buffer. The current cursor position shall be unchanged. If the *count* is larger than the number of characters between the current cursor position and the end of the command line toward which the motion command would move the cursor, this shall not be considered an error; all of the remaining characters in the aforementioned range shall be yanked.
> - **Y**: Yank the characters from the current cursor position to the end of the line into the save buffer. The current character position shall be unchanged.
> - **[***count***]p**: Put a copy of the current contents of the save buffer after the current cursor position. The current cursor position shall be advanced to the last character put from the save buffer. A *count* shall indicate how many copies of the save buffer shall be put.
> - **[***count***]P**: Put a copy of the current contents of the save buffer before the current cursor position. The current cursor position shall be moved to the last character put from the save buffer. A *count* shall indicate how many copies of the save buffer shall be put.
> - **u**: Undo the last command that changed the edit line. This operation shall not undo the copy of any command line to the edit line.
> - **U**: Undo all changes made to the edit line. This operation shall not undo the copy of any command line to the edit line.
> - **[***count***]k**
> - **[***count***]-**: Set the current command line to be the *count*th previous command line in the shell command history. If *count* is not specified, it shall default to 1. The cursor shall be positioned on the first character of the new command. If a **k** or **-** command would retreat past the maximum number of commands in effect for this shell (affected by the *HISTSIZE* environment variable), the terminal shall be alerted, and the command shall have no effect.
> - **[***count***]j**
> - **[***count***]+**: Set the current command line to be the *count*th next command line in the shell command history. If *count* is not specified, it shall default to 1. The cursor shall be positioned on the first character of the new command. If a **j** or **+** command advances past the edit line, the current command line shall be restored to the edit line and the terminal shall be alerted.
> - **[***number***]G**: Set the current command line to be the oldest command line stored in the shell command history. With a number *number*, set the current command line to be the command line *number* in the history. If command line *number* does not exist, the terminal shall be alerted and the command line shall not be changed.
> - **/***pattern*`<newline>`: Move backwards through the command history, searching for the specified pattern, beginning with the previous command line. Patterns use the pattern matching notation described in [*2.14 Pattern Matching Notation*](docs/posix/md/utilities/V3_chap02.md#214-pattern-matching-notation) , except that the `'^'` character shall have special meaning when it appears as the first character of *pattern* . In this case, the `'^'` is discarded and the characters after the `'^'` shall be matched only at the beginning of a line. Commands in the command history shall be treated as strings, not as filenames. If the pattern is not found, the current command line shall be unchanged and the terminal shall be alerted. If it is found in a previous line, the current command line shall be set to that line and the cursor shall be set to the first character of the new command line. If *pattern* is empty, the last non-empty pattern provided to **/** or **?** shall be used. If there is no previous non-empty pattern, the terminal shall be alerted and the current command line shall remain unchanged.
> - **?***pattern*`<newline>`: Move forwards through the command history, searching for the specified pattern, beginning with the next command line. Patterns use the pattern matching notation described in [*2.14 Pattern Matching Notation*](docs/posix/md/utilities/V3_chap02.md#214-pattern-matching-notation) , except that the `'^'` character shall have special meaning when it appears as the first character of *pattern* . In this case, the `'^'` is discarded and the characters after the `'^'` shall be matched only at the beginning of a line. Commands in the command history shall be treated as strings, not as filenames. If the pattern is not found, the current command line shall be unchanged and the terminal shall be alerted. If it is found in a following line, the current command line shall be set to that line and the cursor shall be set to the fist character of the new command line. If *pattern* is empty, the last non-empty pattern provided to **/** or **?** shall be used. If there is no previous non-empty pattern, the terminal shall be alerted and the current command line shall remain unchanged.
> - **n**: Repeat the most recent **/** or **?** command. If there is no previous **/** or **?**, the terminal shall be alerted and the current command line shall remain unchanged.
> - **N**: Repeat the most recent **/** or **?** command, reversing the direction of the search. If there is no previous **/** or **?**, the terminal shall be alerted and the current command line shall remain unchanged.

#### EXIT STATUS

> The following exit values shall be returned:
>
> - 0: The script to be executed consisted solely of zero or more blank lines or comments, or both.
> - 1-125: A non-interactive shell detected an error other than *command_file* not found, *command_file* not executable, or an unrecoverable read error while reading commands (except from the *file* operand of the [*dot*](docs/posix/md/utilities/dot.md) special built-in); including but not limited to syntax, redirection, or variable assignment errors.
> - 126: A specified *command_file* could not be executed due to an [ENOEXEC] error (see [*2.9.1.4 Command Search and Execution*](docs/posix/md/utilities/V3_chap02.md#2914-command-search-and-execution), item 2).
> - 127: A specified *command_file* could not be found by a non-interactive shell.
> - 128: An unrecoverable read error was detected while reading commands, except from the *file* operand of the [*dot*](docs/posix/md/utilities/dot.md) special built-in.
>
> Otherwise, the shell shall terminate in the same manner as for an [*exit*](docs/posix/md/utilities/V3_chap02.md#exit) command with no operands, unless the last command the shell invoked was executed without forking, in which case the wait status seen by the parent process of the shell shall be the wait status of the last command the shell invoked. See the [*exit*](docs/posix/md/utilities/V3_chap02.md#exit) utility in [*2.15 Special Built-In Utilities*](docs/posix/md/utilities/V3_chap02.md#215-special-built-in-utilities).

#### CONSEQUENCES OF ERRORS

> See [*2.8.1 Consequences of Shell Errors*](docs/posix/md/utilities/V3_chap02.md#281-consequences-of-shell-errors).

---

*The following sections are informative.*

#### APPLICATION USAGE

> Standard input and standard error are the files that determine whether a shell is interactive when **-i** is not specified. For example:
>
> ```
> sh > file
> ```
>
> and:
>
> ```
> sh 2> file
> ```
>
> create interactive and non-interactive shells, respectively. Although both accept terminal input, the results of error conditions are different, as described in [*2.8.1 Consequences of Shell Errors*](docs/posix/md/utilities/V3_chap02.md#281-consequences-of-shell-errors); in the second example a redirection error encountered by a special built-in utility aborts the shell.
>
> *sh* **-n** can be used to check for many syntax errors without waiting for *complete_commands* to be executed, but may be fooled into declaring false positives or missing actual errors that would occur when the shell actually evaluates [*eval*](docs/posix/md/utilities/eval.md) commands present in the script, or if there are [*alias*](docs/posix/md/utilities/alias.md) (or [*unalias*](docs/posix/md/utilities/unalias.md)) commands in the script that would alter the syntax of commands that use the affected aliases.
>
> A conforming application must protect its first operand, if it starts with a `<plus-sign>`, by preceding it with the `"--"` argument that denotes the end of the options.
>
> Applications should note that the standard *PATH* to the shell cannot be assumed to be either **/bin/sh** or **/usr/bin/sh**, and should be determined by interrogation of the *PATH* returned by [*getconf*](docs/posix/md/utilities/getconf.md) *PATH ,* ensuring that the returned pathname is an absolute pathname and not a shell built-in.
>
> For example, to determine the location of the standard *sh* utility:
>
> ```
> command -v sh
> ```
>
> On some implementations this might return:
>
> ```
> /usr/xpg4/bin/sh
> ```
>
> Furthermore, on systems that support executable scripts (the `"#!"` construct), it is recommended that applications using executable scripts install them using [*getconf*](docs/posix/md/utilities/getconf.md) *PATH* to determine the shell pathname and update the `"#!"` script appropriately as it is being installed (for example, with [*sed*](docs/posix/md/utilities/sed.md)). For example:
>
> ```
> #
> # Installation time script to install correct POSIX shell pathname
> #
> # Get list of paths to check
> #
> Sifs=$IFS
> Sifs_set=${IFS+y}
> IFS=:
> set -- $(getconf PATH)
> if [ "$Sifs_set" = y ]
> then
>     IFS=$Sifs
> else
>     unset IFS
> fi
> #
> # Check each path for 'sh'
> #
> for i
> do
>     if [ -x "${i}"/sh ]
>     then
>         Pshell=${i}/sh
>     fi
> done
> #
> # This is the list of scripts to update. They should be of the
> # form '${name}.source' and will be transformed to '${name}'.
> # Each script should begin:
> #
> # #!INSTALLSHELLPATH
> #
> scripts="a b c"
> #
> # Transform each script
> #
> for i in ${scripts}
> do
>     sed -e "s|INSTALLSHELLPATH|${Pshell}|" < ${i}.source > ${i}
> done
> ```

#### EXAMPLES

> 1. Execute a shell command from a string:
>   ```
>   sh -c "cat myfile"
>   ```
> 2. Execute a shell script from a file in the current directory:
>   ```
>   sh my_shell_cmds
>   ```

#### RATIONALE

> The *sh* utility and the [*set*](docs/posix/md/utilities/V3_chap02.md#set) special built-in utility share a common set of options.
>
> The name *IFS* was originally an abbreviation of "Input Field Separators"; however, this name is misleading as the *IFS* characters are actually used as field terminators. One justification for ignoring the contents of *IFS* upon entry to the script, beyond security considerations, is to assist possible future shell compilers. Allowing *IFS* to be imported from the environment prevents many optimizations that might otherwise be performed via dataflow analysis of the script itself.
>
> The text in the STDIN section about non-blocking reads concerns an instance of *sh* that has been invoked, probably by a C-language program, with standard input that has been opened using the O_NONBLOCK flag; see [*open*()](docs/posix/md/functions/open.md) in the System Interfaces volume of POSIX.1-2024. If the shell did not reset this flag, it would immediately terminate because no input data would be available yet and that would be considered the same as end-of-file.
>
> The options associated with a *restricted shell* (command name *rsh* and the **-r** option) were excluded because the standard developers considered that the implied level of security could not be achieved and they did not want to raise false expectations.
>
> On systems that support set-user-ID scripts, a historical trapdoor has been to link a script to the name **-i**. When it is called by a sequence such as:
>
> ```
> sh -
> ```
>
> or by:
>
> ```
> #! usr/bin/sh -
> ```
>
> the historical systems have assumed that no option letters follow. Thus, this volume of POSIX.1-2024 allows the single `<hyphen-minus>` to mark the end of the options, in addition to the use of the regular `"--"` argument, because it was considered that the older practice was so pervasive. An alternative approach is taken by the KornShell, where real and effective user/group IDs must match for an interactive shell; this behavior is specifically allowed by this volume of POSIX.1-2024.
>
> **Note:** There are other problems with set-user-ID scripts that the two approaches described here do not resolve.
>
> The initialization process for the history file can be dependent on the system start-up files, in that they may contain commands that effectively preempt the user's settings of *HISTFILE* and *HISTSIZE .* In some historical shells, the history file is initialized just after the *ENV* file has been processed. Therefore, it is implementation-defined whether changes made to *HISTFILE* after the history file has been initialized are effective.
>
> The default messages for the various *MAIL -related* messages are unspecified because they vary across implementations. Typical messages are:
>
> ```
> "you have mail\n"
> ```
>
> or:
>
> ```
> "you have new mail\n"
> ```
>
> It is important that the descriptions of command line editing refer to the same shell as that in POSIX.1-2024 so that interactive users can also be application programmers without having to deal with programmatic differences in their two environments. It is also essential that the utility name *sh* be specified because this explicit utility name is too firmly rooted in historical practice of application programs for it to change.
>
> Consideration was given to mandating a diagnostic message when attempting to set [*vi*](docs/posix/md/utilities/vi.md)-mode on terminals that do not support command line editing. However, it is not historical practice for the shell to be cognizant of all terminal types and thus be able to detect inappropriate terminals in all cases. Implementations are encouraged to supply diagnostics in this case whenever possible, rather than leaving the user in a state where editing commands work incorrectly.
>
> In early proposals, the KornShell-derived *emacs* mode of command line editing was included, even though the *emacs* editor itself was not. The community of *emacs* proponents was adamant that the full *emacs* editor not be standardized because they were concerned that an attempt to standardize this very powerful environment would encourage vendors to ship strictly conforming versions lacking the extensibility required by the community. The author of the original *emacs* program also expressed his desire to omit the program. Furthermore, there were a number of historical systems that did not include *emacs*, or included it without supporting it, but there were very few that did not include and support [*vi*](docs/posix/md/utilities/vi.md). The shell *emacs* command line editing mode was finally omitted because it became apparent that the KornShell version and the editor being distributed with the GNU system had diverged in some respects. The author of *emacs* requested that the POSIX *emacs* mode either be deleted or have a significant number of unspecified conditions. Although the KornShell author agreed to consider changes to bring the shell into alignment, the standard developers decided to defer specification at that time. At the time, it was assumed that convergence on an acceptable definition would occur for a subsequent draft, but that has not happened, and there appears to be no impetus to do so. In any case, implementations are free to offer additional command line editing modes based on the exact models of editors their users are most comfortable with.
>
> Early proposals had the following list entry in [vi Line Editing Insert Mode](#vi-line-editing-insert-mode):
>
> - `\`: If followed by the *erase* or *kill* character, that character shall be inserted into the input line. Otherwise, the `<backslash>` itself shall be inserted into the input line.
>
> However, this is not actually a feature of *sh* command line editing insert mode, but one of some historical terminal line drivers. Some conforming implementations continue to do this when the [*stty*](docs/posix/md/utilities/stty.md) **iexten** flag is set.
>
> In interactive shells, SIGTERM is ignored so that `kill 0` does not kill the shell, and SIGINT is caught so that [*wait*](docs/posix/md/utilities/wait.md) is interruptible. If the shell does not ignore SIGTTIN, SIGTTOU, and SIGTSTP signals when it is interactive and the **-m** option is not in effect, these signals suspend the shell if it is not a session leader. If it is a session leader, the signals are discarded if they would stop the process, as required by XSH [*2.4.3 Signal Actions*](docs/posix/md/functions/V2_chap02.md#243-signal-actions) for orphaned process groups.
>
> Earlier versions of this standard required that input files to the shell be text files except that line lengths were unlimited. However, that was overly restrictive in relation to the fact that shells can parse a script without a trailing newline, and in relation to a common practice of concatenating a shell script ending with an `exit` or `exec $command` with a binary data payload to form a single-file self-extracting archive.

#### FUTURE DIRECTIONS

> If this utility is directed to create a new directory entry that contains any bytes that have the encoded value of a `<newline>` character, implementations are encouraged to treat this as an error. A future version of this standard may require implementations to treat this as an error.

#### SEE ALSO

> [*2.9.1.4 Command Search and Execution*](docs/posix/md/utilities/V3_chap02.md#2914-command-search-and-execution), [*2. Shell Command Language*](docs/posix/md/utilities/V3_chap02.md#2-shell-command-language), [*cd*](docs/posix/md/utilities/cd.md), [*echo*](docs/posix/md/utilities/echo.md), [*exit*](docs/posix/md/utilities/V3_chap02.md#tag_19_22), [*fc*](docs/posix/md/utilities/fc.md), [*pwd*](docs/posix/md/utilities/pwd.md), [*read*](docs/posix/md/utilities/read.md#tag_20_100), [*set*](docs/posix/md/utilities/V3_chap02.md#tag_19_26), [*stty*](docs/posix/md/utilities/stty.md), [*test*](docs/posix/md/utilities/test.md), [*trap*](docs/posix/md/utilities/V3_chap02.md#tag_19_29), [*umask*](docs/posix/md/utilities/umask.md#tag_20_132), [*vi*](docs/posix/md/utilities/vi.md)
>
> XBD [*8. Environment Variables*](docs/posix/md/basedefs/V1_chap08.md#8-environment-variables), [*12.2 Utility Syntax Guidelines*](docs/posix/md/basedefs/V1_chap12.md#122-utility-syntax-guidelines)
>
> XSH [*dup*()](docs/posix/md/functions/dup.md), [*exec*](docs/posix/md/functions/exec.md#tag_17_129), [*exit*()](docs/posix/md/functions/exit.md#tag_17_130), [*fork*()](docs/posix/md/functions/fork.md), [*getrlimit*()](docs/posix/md/functions/getrlimit.md), [*open*()](docs/posix/md/functions/open.md), [*pipe*()](docs/posix/md/functions/pipe.md), [*signal*()](docs/posix/md/functions/signal.md), [*system*()](docs/posix/md/functions/system.md), [*umask*()](docs/posix/md/functions/umask.md#tag_17_645), [*wait*()](docs/posix/md/functions/wait.md#tag_17_658)

#### CHANGE HISTORY

> First released in Issue 2.

#### Issue 5

> The FUTURE DIRECTIONS section is added.
>
> Text is added to the DESCRIPTION for the Large File Summit proposal.

#### Issue 6

> The Open Group Corrigendum U029/2 is applied, correcting the second SYNOPSIS.
>
> The Open Group Corrigendum U027/3 is applied, correcting a typographical error.
>
> The following new requirements on POSIX implementations derive from alignment with the Single UNIX Specification:
>
> - The option letters derived from the [*set*](docs/posix/md/utilities/V3_chap02.md#set) special built-in are also accepted with a leading `<plus-sign>` (`'+'`).
> - Large file extensions are added:
>     - Pathname expansion does not fail due to the size of a file.
>     - Shell input and output redirections have an implementation-defined offset maximum that is established in the open file description.
>
> In the ENVIRONMENT VARIABLES section, the text "user's home directory" is updated to "directory referred to by the *HOME* environment variable".
>
> Descriptions for the *ENV* and *PWD* environment variables are included to align with the IEEE P1003.2b draft standard.
>
> The normative text is reworded to avoid use of the term "must" for application requirements.

#### Issue 7

> Austin Group Interpretation 1003.1-2001 #098 is applied, changing the definition of *IFS .*
>
> SD5-XCU-ERN-97 is applied, updating the SYNOPSIS.
>
> Changes to the [*pwd*](docs/posix/md/utilities/pwd.md) utility and *PWD* environment variable have been made to match the changes to the [*getcwd*()](docs/posix/md/functions/getcwd.md) function made for Austin Group Interpretation 1003.1-2001 #140.
>
> Minor editorial changes are made to the User Portability Utilities option shading. No normative changes are implied.
>
> Minor changes are made to the install script example in the APPLICATION USAGE section.
>
> POSIX.1-2008, Technical Corrigendum 1, XCU/TC1-2008/0137 [152], XCU/TC1-2008/0138 [347], XCU/TC1-2008/0139 [347], XCU/TC1-2008/0140 [347], XCU/TC1-2008/0141 [299], and XCU/TC1-2008/0142 [347] are applied.
>
> POSIX.1-2008, Technical Corrigendum 2, XCU/TC2-2008/0175 [584], XCU/TC2-2008/0176 [584], XCU/TC2-2008/0177 [718], XCU/TC2-2008/0178 [884], XCU/TC2-2008/0179 [809], XCU/TC2-2008/0180 [884], and XCU/TC2-2008/0181 [584] are applied.

#### Issue 8

> Austin Group Defect 51 is applied, changing the EXIT STATUS section.
>
> Austin Group Defect 251 is applied, encouraging implementations to disallow the creation of filenames containing any bytes that have the encoded value of a `<newline>` character.
>
> Austin Group Defect 981 is applied, removing a reference to the [*set*](docs/posix/md/utilities/V3_chap02.md#set) **-o** *nolog* option from the RATIONALE section.
>
> Austin Group Defect 1006 is applied, changing the description of the *ENV* environment variable.
>
> Austin Group Defect 1055 is applied, adding a paragraph about the **-n** option to the APPLICATION USAGE section.
>
> Austin Group Defect 1063 is applied, adding OB shading to the **-h** option and adding it to the list of options that are described as part of the [*set*](docs/posix/md/utilities/V3_chap02.md#set) utility.
>
> Austin Group Defect 1122 is applied, changing the description of *NLSPATH .*
>
> Austin Group Defect 1250 is applied, changing the INPUT FILES section.
>
> Austin Group Defect 1266 is applied, clarifying the circumstances under which the shell is considered to be interactive.
>
> Austin Group Defect 1267 is applied, changing the ENVIRONMENT VARIABLES section to remove the UP shading from *HOME* and add it to *HISTSIZE .*
>
> Austin Group Defect 1519 is applied, making the behavior explicitly unspecified if the **-o** or **+o** option is specified without an option-argument.
>
> Austin Group Defect 1629 is applied, changing the EXIT STATUS section.

*End of informative text.*

### Tests

#### Test: -c runs command_string with $0 and positional params

The -c option causes the shell to execute the command_string operand. The first argument after command_string is assigned to special parameter $0 (command_name), and subsequent arguments become positional parameters $1, $2, etc.

```
begin test "-c runs command_string with $0 and positional params"
  script
    $SHELL -c 'echo "args: $0 $1 $2"' zero one two
  expect
    stdout "args: zero one two"
    stderr ""
    exit_code 0
end test "-c runs command_string with $0 and positional params"
```

#### Test: -c sets $0 to command_name operand

When -c is used, the command_name operand (first argument after command_string) is assigned to special parameter $0. This test verifies $0 reflects the explicitly provided command_name.

```
begin test "-c sets $0 to command_name operand"
  script
    $SHELL -c 'echo "$0"' myname
  expect
    stdout "myname"
    stderr ""
    exit_code 0
end test "-c sets $0 to command_name operand"
```

#### Test: commands from stdin act like -s

When no operands are provided and -c is not specified, the -s option is assumed and the shell reads commands from standard input.

```
begin test "commands from stdin act like -s"
  script
    echo 'echo stdin_test' | $SHELL
  expect
    stdout "stdin_test"
    stderr ""
    exit_code 0
end test "commands from stdin act like -s"
```

#### Test: stdin not consumed entirely if a command needs it

The standard requires that when the shell reads commands from standard input and invokes a command that also uses standard input, the file pointer shall point directly after the command the shell has read. The shell must not consume characters intended for the invoked command.

```
begin test "stdin not consumed entirely if a command needs it"
  script
    (echo 'read line'; echo 'input_for_read'; echo 'echo "got: $line"') | $SHELL
  expect
    stdout "got: input_for_read"
    stderr ""
    exit_code 0
end test "stdin not consumed entirely if a command needs it"
```

#### Test: $0 is shell path when reading from stdin

When sh reads from standard input (no command_file operand), special parameter $0 shall be set to the value of the first argument passed to sh from its parent, which is normally the pathname used to execute the sh utility.

```
begin test "$0 is shell path when reading from stdin"
  script
    echo 'echo $0' | $SHELL
  expect
    stdout ".*sh.*"
    stderr ""
    exit_code 0
end test "$0 is shell path when reading from stdin"
```

#### Test: shell enables blocking reads on FIFO stdin

If standard input to sh is a FIFO or terminal device set to non-blocking reads, the shell shall enable blocking reads on standard input. This test creates a FIFO, writes commands to it asynchronously, and verifies the shell successfully reads and executes them.

```
begin test "shell enables blocking reads on FIFO stdin"
  script
    mkfifo /tmp/test_fifo_$$
    (sleep 0.1; echo 'echo fifo_ok'; echo 'exit') > /tmp/test_fifo_$$ &
    $SHELL < /tmp/test_fifo_$$
    rm -f /tmp/test_fifo_$$
  expect
    stdout "fifo_ok"
    stderr ""
    exit_code 0
end test "shell enables blocking reads on FIFO stdin"
```

#### Test: sh - ignores the hyphen and reads from stdin

A single hyphen-minus operand shall be treated as the first operand and then ignored. The shell then reads commands from standard input as if -s were specified.

```
begin test "sh - ignores the hyphen and reads from stdin"
  script
    echo 'echo hyphen_test' | $SHELL -
  expect
    stdout "hyphen_test"
    stderr ""
    exit_code 0
end test "sh - ignores the hyphen and reads from stdin"
```

#### Test: +a turns off allexport

The option letters derived from the set special built-in are accepted with a leading plus-sign, meaning the reverse of the option. Here +a disables the allexport (-a) option, so variables assigned after +a are not exported.

```
begin test "+a turns off allexport"
  script
    $SHELL -c 'set -a
    export_on="yes"
    set +a
    export_off="no"
    env | grep "^export_"'
  expect
    stdout "export_on=yes"
    stderr ""
    exit_code 0
end test "+a turns off allexport"
```

#### Test: sh reads script from current working directory without slash

When command_file does not contain a slash character, the shell shall attempt to read that file from the current working directory; the file need not be executable.

```
begin test "sh reads script from current working directory without slash"
  script
    echo 'echo "local_script_executed"' > tmp_local.sh
    $SHELL tmp_local.sh
    rm -f tmp_local.sh
  expect
    stdout "local_script_executed"
    stderr ""
    exit_code 0
end test "sh reads script from current working directory without slash"
```

#### Test: -c command_string with semicolon-separated commands

The command_string operand shall be interpreted as one or more commands. Multiple commands separated by semicolons are executed in sequence.

```
begin test "-c command_string with semicolon-separated commands"
  script
    $SHELL -c 'echo first; echo second'
  expect
    stdout "first\nsecond"
    stderr ""
    exit_code 0
end test "-c command_string with semicolon-separated commands"
```

#### Test: -s reads commands from stdin

The -s option causes the shell to read commands from standard input explicitly, even when operands could otherwise be interpreted as a command_file.

```
begin test "-s reads commands from stdin"
  script
    echo 'echo from_stdin' | $SHELL -s
  expect
    stdout "from_stdin"
    stderr ""
    exit_code 0
end test "-s reads commands from stdin"
```

#### Test: ENV file processed for interactive shell

The ENV environment variable, when and only when an interactive shell is invoked, shall be subjected to parameter expansion and the resulting value used as a pathname of a file containing shell commands to execute in the current environment.

```
begin test "ENV file processed for interactive shell"
  script
    _env_file=$(pwd)/_test_env.sh
    echo 'ENV_LOADED=yes' > $_env_file
    ENV=$_env_file
    export ENV
    $SHELL -i -c 'echo $ENV_LOADED' 2>/dev/null
    rm -f $_env_file
  expect
    stdout "yes"
    stderr ""
    exit_code 0
end test "ENV file processed for interactive shell"
```

#### Test: empty command_string exits zero

If the command_string operand is an empty string, sh shall exit with a zero exit status.

```
begin test "empty command_string exits zero"
  script
    $SHELL -c ''
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "empty command_string exits zero"
```

#### Test: FCEDIT can be set and fc uses it

The FCEDIT variable, when expanded by the shell, determines the default editor for the fc built-in's -e option. Setting FCEDIT to a known command and invoking fc verifies the variable is respected.

```
begin interactive test "FCEDIT can be set and fc uses it"
  spawn -i
  expect "$ "
  send "HISTSIZE=100"
  expect "$ "
  send "echo fcedit_test"
  expect "fcedit_test"
  expect "$ "
  send "FCEDIT=true fc -e true"
  expect "$ "
  sendeof
  wait
end interactive test "FCEDIT can be set and fc uses it"
```

#### Test: History list supports at least 128 entries

The HISTSIZE variable determines the limit on accessible previous commands. The maximum number of commands in the history list shall be at least 128. This test sets HISTSIZE to 128 and verifies a command can be recalled.

```
begin interactive test "History list supports at least 128 entries"
  spawn -i
  expect "$ "
  send "HISTSIZE=128"
  expect "$ "
  send "echo hist_min_size"
  expect "hist_min_size"
  expect "$ "
  send "fc -l -1 -1"
  expect "echo hist_min_size"
  expect "$ "
  sendeof
  wait
end interactive test "History list supports at least 128 entries"
```

#### Test: Oldest history entries are deleted first

As entries are deleted from the history file, they shall be deleted oldest first. With HISTSIZE=3, after entering four commands the oldest (cmd1) should no longer be accessible.

```
begin interactive test "Oldest history entries are deleted first"
  spawn -i
  expect "$ "
  send "HISTSIZE=3"
  expect "$ "
  send "echo cmd1"
  expect "cmd1"
  expect "$ "
  send "echo cmd2"
  expect "cmd2"
  expect "$ "
  send "echo cmd3"
  expect "cmd3"
  expect "$ "
  send "echo cmd4"
  expect "cmd4"
  expect "$ "
  send "fc -l -2 -1 | grep -c cmd1 || true; echo end_hist_check"
  expect "0"
  expect "end_hist_check"
  expect "$ "
  sendeof
  wait
end interactive test "Oldest history entries are deleted first"
```

#### Test: History works when HISTFILE is unwritable

If the shell cannot obtain both read and write access to the history file, it shall use an unspecified mechanism that allows the history to operate properly. Setting HISTFILE to an impossible path must not prevent the shell from functioning.

```
begin interactive test "History works when HISTFILE is unwritable"
  spawn -i
  expect "$ "
  send "HISTFILE=/dev/null/impossible"
  expect "$ "
  send "echo hist_works"
  expect "hist_works"
  expect "$ "
  sendeof
  wait
end interactive test "History works when HISTFILE is unwritable"
```

#### Test: History treats commands as strings not filenames

Commands in the command history shall be treated as strings, not as filenames. A command containing glob characters like *.txt must be stored literally in the history without pathname expansion.

```
begin interactive test "History treats commands as strings not filenames"
  spawn -i
  expect "$ "
  send "HISTSIZE=100"
  expect "$ "
  send "echo *.txt"
  expect "$ "
  send "fc -l -1 -1"
  expect "echo \*\.txt"
  expect "$ "
  sendeof
  wait
end interactive test "History treats commands as strings not filenames"
```

#### Test: MAIL notification on file creation

When MAIL is set, the shell shall inform the user if the file named by the variable is created or if its modification time has changed, by writing a message to stderr prior to the next primary prompt. MAILCHECK controls the check interval.

```
begin interactive test "MAIL notification on file creation"
  spawn -i
  expect "$ "
  send "MAILCHECK=1"
  expect "$ "
  send "MAIL=$HOME/mbox1"
  expect "$ "
  sleep 1100ms
  send "echo created > $HOME/mbox1"
  expect timeout=5s "(mail|Mail|MAIL|you have)"
  expect "$ "
  sendeof
  wait
end interactive test "MAIL notification on file creation"
```

#### Test: MAIL not checked if MAILPATH is set

The user shall be informed about MAIL only if MAIL is set and MAILPATH is not set. The MAILPATH variable takes precedence over MAIL.

```
begin interactive test "MAIL not checked if MAILPATH is set"
  spawn -i
  expect "$ "
  send "bind 'set enable-bracketed-paste off'"
  expect "$ "
  send "MAILCHECK=1"
  expect "$ "
  send "MAILPATH=/tmp/nonexistent_mailpath"
  expect "$ "
  send "MAIL=$HOME/mbox2"
  expect "$ "
  sleep 1100ms
  send "echo data > $HOME/mbox2"
  expect "$ "
  sleep 3s
  send "echo mail_check_done"
  expect "mail_check_done"
  expect "$ "
  sendeof
  wait
end interactive test "MAIL not checked if MAILPATH is set"
```

#### Test: MAILCHECK=0 checks at every prompt

If MAILCHECK is set to zero, the shell shall check for mail before issuing each primary prompt.

```
begin interactive test "MAILCHECK=0 checks at every prompt"
  spawn -i
  expect "$ "
  send "MAILCHECK=0"
  expect "$ "
  send "touch $HOME/mbox3"
  expect "$ "
  send "MAIL=$HOME/mbox3"
  expect "$ "
  sleep 1100ms
  send "echo data >> $HOME/mbox3"
  expect timeout=5s "(mail|Mail|MAIL|you have)"
  expect "$ "
  sendeof
  wait
end interactive test "MAILCHECK=0 checks at every prompt"
```

#### Test: MAILPATH with custom message

MAILPATH provides a colon-separated list of pathnames. Each pathname can be followed by % and a string that is subjected to parameter expansion and written to stderr when the file's modification time changes.

```
begin interactive test "MAILPATH with custom message"
  spawn -i
  expect "$ "
  send "MAILCHECK=0"
  expect "$ "
  send "touch $HOME/mp1"
  expect "$ "
  send "MAILPATH=\"$HOME/mp1%custom msg here:$HOME/mp2\""
  expect "$ "
  sleep 1100ms
  send "echo data >> $HOME/mp1"
  expect timeout=5s "custom msg here"
  expect "$ "
  sendeof
  wait
end interactive test "MAILPATH with custom message"
```

#### Test: MAILPATH percent escaping

If a % character in a MAILPATH pathname is preceded by a backslash, it shall be treated as a literal % in the pathname rather than as a message separator.

```
begin interactive test "MAILPATH percent escaping"
  spawn -i
  expect "$ "
  send "MAILPATH='/tmp/file\\%name'"
  expect "$ "
  send "echo ok"
  expect "ok"
  expect "$ "
  sendeof
  wait
end interactive test "MAILPATH percent escaping"
```

#### Test: MAILCHECK defaults to 600

The default value of MAILCHECK shall be 600 seconds.

```
begin interactive test "MAILCHECK defaults to 600"
  spawn -i
  expect "$ "
  send "echo $MAILCHECK"
  expect "600"
  expect "$ "
  sendeof
  wait
end interactive test "MAILCHECK defaults to 600"
```

#### Test: shell script inherits exit status of last command

The shell shall terminate in the same manner as for an exit command with no operands — the exit status is that of the last command executed.

```
begin test "shell script inherits exit status of last command"
  script
    echo "exit 42" > tmp_script.sh
    $SHELL tmp_script.sh
    rc=$?
    rm -f tmp_script.sh
    echo "$rc"
  expect
    stdout "42"
    stderr ""
    exit_code 0
end test "shell script inherits exit status of last command"
```

#### Test: ENV is ignored for non-interactive shell

ENV is processed when and only when an interactive shell is invoked. A non-interactive shell (e.g. sh -c) must not execute the ENV file.

```
begin test "ENV is ignored for non-interactive shell"
  script
    _env_file=_test_env_noninteractive.sh
    echo 'echo env_ran_noninteractive' > "$_env_file"
    ENV=$_env_file $SHELL -c 'echo done'
    rm -f "$_env_file"
  expect
    stdout "done"
    stderr ""
    exit_code 0
end test "ENV is ignored for non-interactive shell"
```

#### Test: PS1 prompt text is emitted to stderr stream

Except as otherwise stated, standard error shall be used only for diagnostic messages. The primary prompt (PS1) in an interactive shell is written to stderr, not stdout.

```
begin test "PS1 prompt text is emitted to stderr stream"
  script
    $SHELL -i > ps1_stdout.txt 2> ps1_stderr.txt <<'EOF'
    PS1='ps1_stream_marker> '
    :
    exit
    EOF
    grep -q 'ps1_stream_marker> ' ps1_stderr.txt && echo stderr_ok || echo stderr_missing
    grep -q 'ps1_stream_marker> ' ps1_stdout.txt && echo stdout_leak || echo stdout_clean
    rm -f ps1_stdout.txt ps1_stderr.txt
  expect
    stdout "stderr_ok\nstdout_clean"
    stderr ""
    exit_code 0
end test "PS1 prompt text is emitted to stderr stream"
```

#### Test: PS2 prompt text is emitted to stderr stream

The continuation prompt (PS2) in an interactive shell, displayed when a command spans multiple lines, is written to stderr, not stdout.

```
begin test "PS2 prompt text is emitted to stderr stream"
  script
    $SHELL -i > ps2_stdout.txt 2> ps2_stderr.txt <<'OUTER'
    PS2='cont> '
    echo 'hello
    world'
    exit
    OUTER
    grep -q 'cont> ' ps2_stderr.txt && echo stderr_ok || echo stderr_missing
    grep -q 'cont> ' ps2_stdout.txt && echo stdout_leak || echo stdout_clean
    rm -f ps2_stdout.txt ps2_stderr.txt
  expect
    stdout "stderr_ok\nstdout_clean"
    stderr ""
    exit_code 0
end test "PS2 prompt text is emitted to stderr stream"
```

#### Test: blank lines and comments exit zero

If the input file consists solely of zero or more blank lines and comments, sh shall exit with a zero exit status.

```
begin test "blank lines and comments exit zero"
  script
    printf '
    # just a comment


    ' > tmp_empty.sh; $SHELL tmp_empty.sh; rm -f tmp_empty.sh
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "blank lines and comments exit zero"
```

#### Test: empty file exits zero

If the input file consists solely of zero bytes (empty), the shell shall exit with a zero exit status, since it contains zero blank lines and comments.

```
begin test "empty file exits zero"
  script
    printf '' > tmp_empty_file.sh
    $SHELL tmp_empty_file.sh
    rm -f tmp_empty_file.sh
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "empty file exits zero"
```

#### Test: -c does not read from stdin

When -c is specified, no commands shall be read from standard input. Data piped into the shell must be ignored.

```
begin test "-c does not read from stdin"
  script
    echo 'echo LEAKED' | $SHELL -c 'echo hello'
  expect
    stdout "hello"
    stderr ""
    exit_code 0
end test "-c does not read from stdin"
```

#### Test: $0 is set to command_file when running script

When a command_file operand is specified, special parameter $0 shall be set to the value of command_file.

```
begin test "$0 is set to command_file when running script"
  script
    echo 'echo "$0"' > tmp_s0.sh
    $SHELL tmp_s0.sh
    rm -f tmp_s0.sh
  expect
    stdout "tmp_s0.sh"
    stderr ""
    exit_code 0
end test "$0 is set to command_file when running script"
```

#### Test: -s sets positional parameters from arguments

When -s is specified, remaining arguments shall be set as positional parameters ($1, $2, etc.).

```
begin test "-s sets positional parameters from arguments"
  script
    echo 'echo "$1 $2"' | $SHELL -s one two
  expect
    stdout "one two"
    stderr ""
    exit_code 0
end test "-s sets positional parameters from arguments"
```

#### Test: command_file arguments become positional parameters

Arguments following the command_file operand shall be set as positional parameters for the script.

```
begin test "command_file arguments become positional parameters"
  script
    echo 'echo "$1 $2"' > tmp_pos.sh
    $SHELL tmp_pos.sh aa bb
    rm -f tmp_pos.sh
  expect
    stdout "aa bb"
    stderr ""
    exit_code 0
end test "command_file arguments become positional parameters"
```

#### Test: -c without command_name defaults $0 to shell path

When -c is used without a command_name operand, special parameter $0 shall be set to the first argument passed to sh from its parent (argv[0]).

```
begin test "-c without command_name defaults $0 to shell path"
  script
    $SHELL -c 'echo "$0"'
  expect
    stdout ".*sh.*"
    stderr ""
    exit_code 0
end test "-c without command_name defaults $0 to shell path"
```

#### Test: exit 127 for command_file not found

A non-interactive shell shall return exit status 127 when the specified command_file could not be found.

```
begin test "exit 127 for command_file not found"
  script
    $SHELL /nonexistent_script_path_xyz 2>/dev/null; echo $?
  expect
    stdout "127"
    stderr ""
    exit_code 0
end test "exit 127 for command_file not found"
```

#### Test: no line length limit enforced

The shell shall not enforce any line length limits when parsing input files.

```
begin test "no line length limit enforced"
  script
    long=$(printf 'x%.0s' $(seq 1 10000))
    printf 'echo %s\n' "$long" > tmp_long.sh
    result=$($SHELL tmp_long.sh)
    printf '%s\n' "${#result}"
    rm -f tmp_long.sh
  expect
    stdout "10000"
    stderr ""
    exit_code 0
end test "no line length limit enforced"
```

#### Test: syntax error exits 1-125

A non-interactive shell that detects a syntax error shall exit with a status in the range 1-125.

```
begin test "syntax error exits 1-125"
  script
    $SHELL -c 'if then fi' 2>/dev/null; echo $?
  expect
    stdout "([1-9]|[1-9][0-9]|1[01][0-9]|12[0-5])"
    stderr ""
    exit_code 0
end test "syntax error exits 1-125"
```

#### Test: interactive shell ignores SIGTERM

If the shell is interactive, SIGTERM signals shall be ignored. Sending SIGTERM to an interactive shell must not terminate it.

```
begin interactive test "interactive shell ignores SIGTERM"
  spawn -i
  expect "$ "
  send "kill -TERM $$"
  expect "$ "
  send "echo survived_sigterm"
  expect "survived_sigterm"
  expect "$ "
  sendeof
  wait
end interactive test "interactive shell ignores SIGTERM"
```

#### Test: interactive here-document prompt goes to stderr stream

When an interactive shell encounters a here-document that requires continuation input, the PS2 prompt is written to stderr, not stdout.

```
begin test "interactive here-document prompt goes to stderr stream"
  script
    $SHELL -i > hd_stdout.txt 2> hd_stderr.txt <<'EOF'
    PS2='heredoc> '
    cat <<EOT
    first
    EOT
    exit
    EOF
    grep -q 'heredoc> ' hd_stderr.txt && echo stderr_ok || echo stderr_missing
    grep -q 'heredoc> ' hd_stdout.txt && echo stdout_leak || echo stdout_clean
    rm -f hd_stdout.txt hd_stderr.txt
  expect
    stdout "stderr_ok\nstdout_clean"
    stderr ""
    exit_code 0
end test "interactive here-document prompt goes to stderr stream"
```

#### Test: history maintained interactively

When sh is being used interactively, it shall maintain a list of commands previously entered from the terminal. This test verifies commands can be recalled via fc.

```
begin interactive test "history maintained interactively"
  spawn -i
  expect "$ "
  send "echo histcheck_1"
  expect "histcheck_1"
  expect "$ "
  send "echo histcheck_2"
  expect "histcheck_2"
  expect "$ "
  send "fc -l -2 -1"
  expect "histcheck_1"
  expect "histcheck_2"
  sendeof
  wait
end interactive test "history maintained interactively"
```

#### Test: user must explicitly exit interactive shell

An interactive shell does not terminate automatically after executing a command; the user must explicitly issue an exit command to end the session.

```
begin interactive test "user must explicitly exit interactive shell"
  spawn -i
  expect "$ "
  send "echo still_here"
  expect "still_here"
  expect "$ "
  send "exit"
  wait
end interactive test "user must explicitly exit interactive shell"
```

#### Test: vi mode basic insert

The command `set -o vi` shall enable vi-mode editing and place the shell into vi insert mode. Characters typed in insert mode are inserted into the command line.

```
begin interactive test "vi mode basic insert"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  send "echo vi_mode_works"
  expect "vi_mode_works"
  expect "$ "
  sendeof
  wait
end interactive test "vi mode basic insert"
```

#### Test: vi insert mode is default after command

Upon entering sh and after termination of the previous command, sh shall be in insert mode. Consecutive commands can be typed without explicitly re-entering insert mode.

```
begin interactive test "vi insert mode is default after command"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  send "echo first"
  expect "first"
  expect "$ "
  send "echo second"
  expect "second"
  expect "$ "
  sendeof
  wait
end interactive test "vi insert mode is default after command"
```

#### Test: ESC switches to command mode

Typing an escape character shall switch sh into command mode. In command mode, editing commands like `x` (delete character) can be used before pressing Enter to execute.

```
begin interactive test "ESC switches to command mode"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 68 65 6c 6c 6f 78
  sendraw 1b
  sleep 100ms
  sendraw 78
  sleep 100ms
  sendraw 0a
  expect "hello"
  expect "$ "
  sendeof
  wait
end interactive test "ESC switches to command mode"
```

#### Test: replace character r

In vi command mode, `r` followed by a character replaces the character at the current cursor position with the specified character.

```
begin interactive test "replace character r"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 68 65 6c 6c 78
  sendraw 1b
  sleep 100ms
  sendraw 72 6f
  sleep 100ms
  sendraw 0a
  expect "hello"
  expect "$ "
  sendeof
  wait
end interactive test "replace character r"
```

#### Test: case inversion tilde

The `~` command in vi command mode converts lowercase to uppercase and vice versa at the current cursor position, then advances the cursor by one character.

```
begin interactive test "case inversion tilde"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 42 43
  sendraw 1b
  sleep 100ms
  sendraw 30 77
  sleep 50ms
  sendraw 7e 7e 7e
  sleep 100ms
  sendraw 0a
  expect "Abc"
  expect "$ "
  sendeof
  wait
end interactive test "case inversion tilde"
```

#### Test: cursor movement h and l

In vi command mode, `h` moves the cursor one position to the left and `l` moves it one position to the right. This test uses `h` to back up and `r` to replace a character at that position.

```
begin interactive test "cursor movement h and l"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 62 63 64
  sendraw 1b
  sleep 100ms
  sendraw 68 68 72 61
  sleep 100ms
  sendraw 0a
  expect "aacd"
  expect "$ "
  sendeof
  wait
end interactive test "cursor movement h and l"
```

#### Test: word movement w and b

In vi command mode, `w` moves to the start of the next word and `b` moves to the beginning of the current or previous word.

```
begin interactive test "word movement w and b"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 6f 6e 65 20 74 77 6f
  sendraw 1b
  sleep 100ms
  sendraw 62 72 58
  sleep 100ms
  sendraw 0a
  expect "one Xwo"
  expect "$ "
  sendeof
  wait
end interactive test "word movement w and b"
```

#### Test: delete character x

The `x` command deletes the character at the current cursor position and places it in the save buffer.

```
begin interactive test "delete character x"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 62 63
  sendraw 1b
  sleep 100ms
  sendraw 78
  sleep 100ms
  sendraw 0a
  expect "ab"
  expect "$ "
  sendeof
  wait
end interactive test "delete character x"
```

#### Test: vi command mode redraw preserves prompt

After a vi command-mode edit that triggers a screen redraw (such as `x`
to delete a character), the prompt shall remain visible on the terminal
line before the edited command text.

```
begin interactive test "vi command mode redraw preserves prompt"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 62 63
  sendraw 1b
  sleep 100ms
  sendraw 78
  sleep 100ms
  expect "$ echo ab"
  sendraw 0a
  expect "ab"
  expect "$ "
  sendeof
  wait
end interactive test "vi command mode redraw preserves prompt"
```

#### Test: count prefix 3x

Decimal digits preceding a command letter serve as a count. Here `3x` deletes three characters starting at the cursor position.

```
begin interactive test "count prefix 3x"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 62 63 64 65
  sendraw 1b
  sleep 100ms
  sendraw 33 78
  sleep 100ms
  sendraw 0a
  expect "ab"
  expect "$ "
  sendeof
  wait
end interactive test "count prefix 3x"
```

#### Test: append a inserts after cursor

The `a` command enters insert mode after the current cursor position. Characters entered are inserted before the next character.

```
begin interactive test "append a inserts after cursor"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 63
  sendraw 1b
  sleep 100ms
  sendraw 68 61 62
  sendraw 1b
  sleep 100ms
  sendraw 0a
  expect "abc"
  expect "$ "
  sendeof
  wait
end interactive test "append a inserts after cursor"
```

#### Test: delete with motion dw

The `d` command followed by a motion command deletes characters between the current position and the position resulting from the motion. Here `dw` deletes to the start of the next word.

```
begin interactive test "delete with motion dw"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 62 20 63 64
  sendraw 1b
  sleep 100ms
  sendraw 62
  sleep 50ms
  sendraw 64 77
  sleep 100ms
  sendraw 0a
  expect "ab"
  expect "$ "
  sendeof
  wait
end interactive test "delete with motion dw"
```

#### Test: dd clears entire command line

If the motion command following `d` is `d` itself, the entire current command line shall be cleared.

```
begin interactive test "dd clears entire command line"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 68 65 6c 6c 6f
  sendraw 1b
  sleep 100ms
  sendraw 64 64
  sleep 100ms
  sendraw 0a
  expect "$ "
  sendeof
  wait
end interactive test "dd clears entire command line"
```

#### Test: delete backward preserves cursor char

When `d` with a backward motion (like `b`) is used, the character under the current cursor position shall not be deleted — only characters between the cursor and the motion target are removed.

```
begin interactive test "delete backward preserves cursor char"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  # Type: echo ab cd (two words after echo)
  sendraw 65 63 68 6f 20 61 62 20 63 64
  sendraw 1b
  sleep 100ms
  # Move to start of "cd", then db (delete backward word "ab ")
  sendraw 77
  sleep 50ms
  sendraw 64 62
  sleep 100ms
  sendraw 0a
  expect "cd"
  expect "$ "
  sendeof
  wait
end interactive test "delete backward preserves cursor char"
```

#### Test: delete with invalid motion alerts

If the motion command following `d` is invalid (not a recognized motion), the terminal shall be alerted, the cursor shall not move, and no text shall be deleted.

```
begin interactive test "delete with invalid motion alerts"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 6f 6b
  sendraw 1b
  sleep 100ms
  # dz is invalid (z is not a motion)
  sendraw 64 7a
  sleep 100ms
  sendraw 0a
  expect "ok"
  expect "$ "
  sendeof
  wait
end interactive test "delete with invalid motion alerts"
```

#### Test: change with motion cc

The `c` command followed by a motion deletes text and enters insert mode. If the motion is `c` itself, the current command line shall be cleared and insert mode entered.

```
begin interactive test "change with motion cc"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 6f 6c 64
  sendraw 1b
  sleep 100ms
  sendraw 63 63
  sleep 50ms
  sendraw 65 63 68 6f 20 6e 65 77
  sendraw 0a
  expect "new"
  expect "$ "
  sendeof
  wait
end interactive test "change with motion cc"
```

#### Test: history navigation k

In vi command mode, `k` sets the current command line to the previous command in the history. The default count is 1.

```
begin interactive test "history navigation k"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  send "echo hist1"
  expect "hist1"
  expect "$ "
  sendraw 1b
  sleep 100ms
  sendraw 6b
  sleep 100ms
  sendraw 0a
  expect "hist1"
  expect "$ "
  sendeof
  wait
end interactive test "history navigation k"
```

#### Test: go to beginning 0 and end dollar

In vi command mode, `0` moves to the first character position and `$` moves to the last character position on the current command line.

```
begin interactive test "go to beginning 0 and end dollar"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 62
  sendraw 1b
  sleep 100ms
  sendraw 30
  sleep 50ms
  sendraw 24
  sleep 50ms
  sendraw 72 5a
  sleep 100ms
  sendraw 0a
  expect "aZ"
  expect "$ "
  sendeof
  wait
end interactive test "go to beginning 0 and end dollar"
```

#### Test: undo u

The `u` command undoes the last command that changed the edit line. After deleting a character with `x`, pressing `u` restores it.

```
begin interactive test "undo u"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 62 63
  sendraw 1b
  sleep 100ms
  sendraw 78
  sleep 50ms
  sendraw 75
  sleep 100ms
  sendraw 0a
  expect "abc"
  expect "$ "
  sendeof
  wait
end interactive test "undo u"
```

#### Test: dot repeat

The `.` command repeats the most recent non-motion command. After `x` deletes one character, `.` deletes another.

```
begin interactive test "dot repeat"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 62 63 64
  sendraw 1b
  sleep 100ms
  sendraw 78
  sleep 50ms
  sendraw 2e
  sleep 100ms
  sendraw 0a
  expect "ab"
  expect "$ "
  sendeof
  wait
end interactive test "dot repeat"
```

#### Test: put p after delete

The `p` command puts a copy of the save buffer after the current cursor position. After deleting a character with `x`, the deleted character can be repositioned with `p`.

```
begin interactive test "put p after delete"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 62 63
  sendraw 1b
  sleep 100ms
  sendraw 78
  sleep 50ms
  sendraw 68
  sleep 50ms
  sendraw 70
  sleep 100ms
  sendraw 0a
  expect "acb"
  expect "$ "
  sendeof
  wait
end interactive test "put p after delete"
```

#### Test: SIGINT in command mode

If sh receives a SIGINT signal in command mode, it shall terminate command line editing on the current command line, reissue the prompt on the next line, and reset the command history so that the interrupted command is not re-entered.

```
begin interactive test "SIGINT in command mode"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 70 61 72 74 69 61 6c
  sendraw 1b
  sleep 100ms
  sendraw 03
  expect "$ "
  send "echo after_sigint"
  expect "after_sigint"
  expect "$ "
  sendeof
  wait
end interactive test "SIGINT in command mode"
```

#### Test: insert mode enters commands into history

When a non-empty command line is executed from insert mode (by pressing Enter), the line shall be entered into the command history and can be recalled with `k`.

```
begin interactive test "insert mode enters commands into history"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  send "echo history_test_entry"
  expect "history_test_entry"
  expect "$ "
  sendraw 1b
  sleep 100ms
  sendraw 6b
  sleep 100ms
  sendraw 0a
  expect "history_test_entry"
  expect "$ "
  sendeof
  wait
end interactive test "insert mode enters commands into history"
```

#### Test: unrecognized command alerts terminal

A character that is not recognized as part of an editing command shall terminate any specific editing command and alert the terminal, but the command line remains unchanged.

```
begin interactive test "unrecognized command alerts terminal"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 6f 6b
  sendraw 1b
  sleep 100ms
  sendraw 5a
  sleep 100ms
  sendraw 0a
  expect "ok"
  expect "$ "
  sendeof
  wait
end interactive test "unrecognized command alerts terminal"
```

#### Test: insert mode backspace erases

The erase character (backspace) in insert mode deletes the character previous to the current cursor position. Characters shall be erased from both the screen and the buffer.

```
begin interactive test "insert mode backspace erases"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 62 58 7f
  sendraw 0a
  expect "ab"
  expect "$ "
  sendeof
  wait
end interactive test "insert mode backspace erases"
```

#### Test: EOF at beginning of line exits shell

The end-of-file character shall be interpreted as the end of input when it occurs at the beginning of an input line. Sending EOF on an empty prompt line terminates the interactive shell.

```
begin interactive test "EOF at beginning of line exits shell"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendeof
  wait
end interactive test "EOF at beginning of line exits shell"
```

#### Test: edit line semantics modify from history

If the current line is not the edit line, any command that modifies it shall cause its content to replace the edit line content, and it becomes the new edit line. The modification is then performed on the edit line.

```
begin interactive test "edit line semantics modify from history"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  send "echo from_hist"
  expect "from_hist"
  expect "$ "
  sendraw 1b
  sleep 100ms
  sendraw 6b
  sleep 100ms
  sendraw 24
  sleep 50ms
  sendraw 72 58
  sleep 100ms
  sendraw 0a
  expect "from_hisX"
  expect "$ "
  sendeof
  wait
end interactive test "edit line semantics modify from history"
```

#### Test: l count overflow clamps to last character

If the count for `l` is larger than the number of characters after the cursor, this shall not be considered an error; the cursor shall advance to the last character on the line.

```
begin interactive test "l count overflow clamps to last character"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 62
  sendraw 1b
  sleep 100ms
  sendraw 39 6c
  sleep 100ms
  sendraw 0a
  expect "ab"
  expect "$ "
  sendeof
  wait
end interactive test "l count overflow clamps to last character"
```

#### Test: tilde count overflow

If the `~` count is larger than the number of characters after the cursor, this shall not be considered an error; the cursor shall advance to the last character on the line and all reachable characters are case-converted. Known bash non-compliance #4: bash only toggles one character instead of applying the count.

```
begin interactive test "tilde count overflow"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 42
  sendraw 1b
  sleep 100ms
  sendraw 30 77
  sleep 50ms
  sendraw 39 7e
  sleep 100ms
  sendraw 0a
  expect "Ab"
  expect "$ "
  sendeof
  wait
end interactive test "tilde count overflow"
```

#### Test: dot repeat count propagation

If the previous command was preceded by a count and no count is given on the `.` command, the count from the previous command is included as part of the repeated command. Here `2x` followed by `.` deletes 2 more characters.

```
begin interactive test "dot repeat count propagation"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 62 63 64 65 66
  sendraw 1b
  sleep 100ms
  sendraw 32 78
  sleep 100ms
  sendraw 2e
  sleep 100ms
  sendraw 0a
  expect "ab"
  expect "$ "
  sendeof
  wait
end interactive test "dot repeat count propagation"
```

#### Test: h count overflow

If the count for `h` is larger than the number of characters before the cursor, this shall not be considered an error; the cursor shall move to the first character on the line.

```
begin interactive test "h count overflow"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 62
  sendraw 1b
  sleep 100ms
  sendraw 30 77 6c
  sleep 50ms
  sendraw 39 39 68
  sleep 50ms
  sendraw 72 5a
  sleep 100ms
  sendraw 0a
  expect "Zcho"
  expect "$ "
  sendeof
  wait
end interactive test "h count overflow"
```

#### Test: bigword forward W

The `W` command moves to the start of the next bigword. A bigword is delimited only by blank characters, treating punctuation as part of the word.

```
begin interactive test "bigword forward W"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 2e 62 20 63 2e 64
  sendraw 1b
  sleep 100ms
  sendraw 30
  sleep 50ms
  sendraw 57 57
  sleep 50ms
  sendraw 72 5a
  sleep 100ms
  sendraw 0a
  expect "a\.b Z\.d"
  expect "$ "
  sendeof
  wait
end interactive test "bigword forward W"
```

#### Test: bigword backward B

The `B` command moves to the beginning of the current or previous bigword.

```
begin interactive test "bigword backward B"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 2e 62 20 63 2e 64
  sendraw 1b
  sleep 100ms
  sendraw 42
  sleep 50ms
  sendraw 72 5a
  sleep 100ms
  sendraw 0a
  expect "a\.b Z\.d"
  expect "$ "
  sendeof
  wait
end interactive test "bigword backward B"
```

#### Test: pipe column movement

The `|` command moves to the count-th character position on the current command line. The first character position is numbered 1.

```
begin interactive test "pipe column movement"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 62 63 64 65
  sendraw 1b
  sleep 100ms
  sendraw 33 7c
  sleep 50ms
  sendraw 72 5a
  sleep 100ms
  sendraw 0a
  expect "ecZo"
  expect "$ "
  sendeof
  wait
end interactive test "pipe column movement"
```

#### Test: find forward f

The `f` command followed by a character moves the cursor to the first occurrence of that character after the current cursor position.

```
begin interactive test "find forward f"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 62 63 62
  sendraw 1b
  sleep 100ms
  sendraw 30
  sleep 50ms
  sendraw 66 62
  sleep 50ms
  sendraw 72 5a
  sleep 100ms
  sendraw 0a
  expect "Zcb"
  expect "$ "
  sendeof
  wait
end interactive test "find forward f"
```

#### Test: find backward F

The `F` command followed by a character moves the cursor to the first occurrence of that character before the current cursor position.

```
begin interactive test "find backward F"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 62 63
  sendraw 1b
  sleep 100ms
  sendraw 46 61
  sleep 50ms
  sendraw 72 5a
  sleep 100ms
  sendraw 0a
  expect "Zbc"
  expect "$ "
  sendeof
  wait
end interactive test "find backward F"
```

#### Test: find forward stop before t

The `t` command moves to the character before the first occurrence of the specified character after the current cursor position.

```
begin interactive test "find forward stop before t"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 62 64
  sendraw 1b
  sleep 100ms
  sendraw 30 77
  sleep 50ms
  sendraw 74 64
  sleep 50ms
  sendraw 72 5a
  sleep 100ms
  sendraw 0a
  expect "aZd"
  expect "$ "
  sendeof
  wait
end interactive test "find forward stop before t"
```

#### Test: find backward stop after T

The `T` command moves to the character after the first occurrence of the specified character before the current cursor position.

```
begin interactive test "find backward stop after T"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 62 64
  sendraw 1b
  sleep 100ms
  sendraw 54 61
  sleep 50ms
  sendraw 72 5a
  sleep 100ms
  sendraw 0a
  expect "aZd"
  expect "$ "
  sendeof
  wait
end interactive test "find backward stop after T"
```

#### Test: repeat find semicolon

The `;` command repeats the most recent `f`, `F`, `t`, or `T` command. Any number argument on the previous command shall be ignored.

```
begin interactive test "repeat find semicolon"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 62 61 62
  sendraw 1b
  sleep 100ms
  sendraw 30
  sleep 50ms
  sendraw 66 61
  sleep 50ms
  sendraw 3b
  sleep 50ms
  sendraw 72 5a
  sleep 100ms
  sendraw 0a
  expect "abZb"
  expect "$ "
  sendeof
  wait
end interactive test "repeat find semicolon"
```

#### Test: reverse repeat find comma

The `,` command repeats the most recent `f`, `F`, `t`, or `T` command but reverses the direction of the search.

```
begin interactive test "reverse repeat find comma"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 62 61 62
  sendraw 1b
  sleep 100ms
  sendraw 30
  sleep 50ms
  sendraw 66 61
  sleep 50ms
  sendraw 3b
  sleep 50ms
  sendraw 2c
  sleep 50ms
  sendraw 72 5a
  sleep 100ms
  sendraw 0a
  expect "Zbab"
  expect "$ "
  sendeof
  wait
end interactive test "reverse repeat find comma"
```

#### Test: change word cw

The `cw` command deletes characters from the cursor to the start of the next word and enters insert mode. The character under the cursor is included in the deletion when the motion moves toward the end of the line.

```
begin interactive test "change word cw"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 62 63 64
  sendraw 1b
  sleep 100ms
  sendraw 30 77
  sleep 50ms
  sendraw 63 77
  sleep 50ms
  sendraw 58 59
  sendraw 1b
  sleep 100ms
  sendraw 0a
  expect "XY"
  expect "$ "
  sendeof
  wait
end interactive test "change word cw"
```

#### Test: change word count overflow 9cw

If the count for `c` with a motion is larger than the number of characters between the cursor and the end of the line, all remaining characters are deleted and insert mode is entered.

```
begin interactive test "change word count overflow 9cw"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 62
  sendraw 1b
  sleep 100ms
  sendraw 30 77
  sleep 50ms
  sendraw 39 63 77
  sleep 50ms
  sendraw 5a
  sendraw 1b
  sleep 100ms
  sendraw 0a
  expect "Z"
  expect "$ "
  sendeof
  wait
end interactive test "change word count overflow 9cw"
```

#### Test: delete x count overflow 9x

If the count for `x` is larger than the number of characters after the cursor, all characters from the cursor to the end of the line shall be deleted.

```
begin interactive test "delete x count overflow 9x"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 62
  sendraw 1b
  sleep 100ms
  sendraw 30 77
  sleep 50ms
  sendraw 39 78
  sleep 100ms
  sendraw 0a
  expect "$ "
  sendeof
  wait
end interactive test "delete x count overflow 9x"
```

#### Test: X deletes before cursor

The `X` command deletes the character before the current cursor position. The character under the current cursor position shall not change.

```
begin interactive test "X deletes before cursor"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 62 63
  sendraw 1b
  sleep 100ms
  sendraw 58
  sleep 100ms
  sendraw 0a
  expect "ac"
  expect "$ "
  sendeof
  wait
end interactive test "X deletes before cursor"
```

#### Test: X on first char no effect

If the cursor is positioned on the first character of the line, the `X` command shall have no effect and the terminal shall be alerted.

```
begin interactive test "X on first char no effect"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 62
  sendraw 1b
  sleep 100ms
  sendraw 30
  sleep 50ms
  sendraw 58
  sleep 100ms
  sendraw 0a
  expect "ab"
  expect "$ "
  sendeof
  wait
end interactive test "X on first char no effect"
```

#### Test: X count overflow 99X

If the count for `X` is larger than the number of characters before the cursor, all characters from before the cursor to the beginning of the line shall be deleted.

```
begin interactive test "X count overflow 99X"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 62 63 64
  sendraw 1b
  sleep 100ms
  sendraw 39 39 58
  sleep 100ms
  sendraw 0a
  expect "$ "
  sendeof
  wait
end interactive test "X count overflow 99X"
```

#### Test: yank yw cursor unchanged

The `y` command followed by a motion copies characters into the save buffer. The current cursor position shall be unchanged after yanking.

```
begin interactive test "yank yw cursor unchanged"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 62 63
  sendraw 1b
  sleep 100ms
  sendraw 30
  sleep 50ms
  sendraw 79 77
  sleep 50ms
  sendraw 72 5a
  sleep 100ms
  sendraw 0a
  expect "Zcho"
  expect "$ "
  sendeof
  wait
end interactive test "yank yw cursor unchanged"
```

#### Test: history k past HISTSIZE boundary

If a `k` or `-` command would retreat past the maximum number of commands in effect (controlled by HISTSIZE), the terminal shall be alerted and the command shall have no effect.

```
begin interactive test "history k past HISTSIZE boundary"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  send "echo only_cmd"
  expect "only_cmd"
  expect "$ "
  sendraw 1b
  sleep 100ms
  sendraw 6b
  sleep 100ms
  sendraw 6b
  sleep 100ms
  sendraw 0a
  expect "only_cmd"
  expect "$ "
  sendeof
  wait
end interactive test "history k past HISTSIZE boundary"
```

#### Test: history j past edit line

If a `j` or `+` command advances past the edit line, the current command line shall be restored to the edit line and the terminal shall be alerted.

```
begin interactive test "history j past edit line"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  send "echo hist_a"
  expect "hist_a"
  expect "$ "
  sendraw 1b
  sleep 100ms
  sendraw 6b
  sleep 100ms
  sendraw 6a
  sleep 100ms
  sendraw 0a
  expect "$ "
  sendeof
  wait
end interactive test "history j past edit line"
```

#### Test: history G nonexistent line

The `G` command with a number sets the current command line to that numbered history entry. If the line number does not exist, the terminal shall be alerted and the command line shall not be changed.

```
begin interactive test "history G nonexistent line"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  send "echo baseline"
  expect "baseline"
  expect "$ "
  sendraw 1b
  sleep 100ms
  sendraw 39 39 39 39 39 47
  sleep 100ms
  sendraw 0a
  expect "$ "
  sendeof
  wait
end interactive test "history G nonexistent line"
```

#### Test: HISTSIZE unset default

If HISTSIZE is unset, an unspecified default greater than or equal to 128 shall be used. History recall must still function normally.

```
begin interactive test "HISTSIZE unset default"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  send "unset HISTSIZE"
  expect "$ "
  send "echo histtest1"
  expect "histtest1"
  expect "$ "
  sendraw 1b
  sleep 100ms
  sendraw 6b
  sleep 100ms
  sendraw 0a
  expect "histtest1"
  expect "$ "
  sendeof
  wait
end interactive test "HISTSIZE unset default"
```

#### Test: history search backward with slash

The `/pattern` command searches backward through command history for lines matching the pattern. If found, the current command line is set to that line.

```
begin interactive test "history search backward with slash"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  send "echo searchme"
  expect "searchme"
  expect "$ "
  send "echo other"
  expect "other"
  expect "$ "
  sendraw 1b
  sleep 100ms
  sendraw 2f 73 65 61 72 63 68 0a
  sleep 200ms
  sendraw 0a
  expect "searchme"
  expect "$ "
  sendeof
  wait
end interactive test "history search backward with slash"
```

#### Test: history search nonexistent pattern

If the search pattern is not found in the history, the current command line shall be unchanged and the terminal shall be alerted.

```
begin interactive test "history search nonexistent pattern"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  send "echo first"
  expect "first"
  expect "$ "
  sendraw 1b
  sleep 100ms
  sendraw 2f 7a 7a 7a 7a 7a 7a 0a
  sleep 200ms
  sendraw 0a
  expect "$ "
  sendeof
  wait
end interactive test "history search nonexistent pattern"
```

#### Test: move to end of word e

The `e` command moves the cursor to the end of the current word. If already at the end of a word, it moves to the end of the next word.

```
begin interactive test "move to end of word e"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 58 62 20 63 64
  sendraw 1b
  sleep 100ms
  sendraw 30
  sleep 50ms
  sendraw 77
  sleep 50ms
  sendraw 65
  sleep 50ms
  sendraw 72 5a
  sleep 100ms
  sendraw 0a
  expect "XZ cd"
  expect "$ "
  sendeof
  wait
end interactive test "move to end of word e"
```

#### Test: move to end of bigword E

The `E` command moves to the end of the current bigword. A bigword is delimited only by blank characters.

```
begin interactive test "move to end of bigword E"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 2d 62 20 63 64
  sendraw 1b
  sleep 100ms
  sendraw 30
  sleep 50ms
  sendraw 57
  sleep 50ms
  sendraw 45
  sleep 50ms
  sendraw 72 5a
  sleep 100ms
  sendraw 0a
  expect "a-Z cd"
  expect "$ "
  sendeof
  wait
end interactive test "move to end of bigword E"
```

#### Test: put before cursor P

The `P` command puts a copy of the save buffer before the current cursor position. The cursor is moved to the last character put from the save buffer.

```
begin interactive test "put before cursor P"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 62 63
  sendraw 1b
  sleep 100ms
  sendraw 78
  sleep 50ms
  sendraw 68
  sleep 50ms
  sendraw 50
  sleep 100ms
  sendraw 0a
  expect "cab"
  expect "$ "
  sendeof
  wait
end interactive test "put before cursor P"
```

#### Test: history search forward with question mark

The `?pattern` command searches forward through command history for the specified pattern. If found, the current command line is set to that line.

```
begin interactive test "history search forward with question mark"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  send "echo findme"
  expect "findme"
  expect "$ "
  send "echo other"
  expect "other"
  expect "$ "
  sendraw 1b
  sleep 100ms
  sendraw 3f 66 69 6e 64 6d 65 0a
  sleep 200ms
  sendraw 0a
  expect "findme"
  expect "$ "
  sendeof
  wait
end interactive test "history search forward with question mark"
```

#### Test: repeat search n

The `n` command repeats the most recent `/` or `?` search in the same direction. After `/` finds a match, `n` searches further backward to find the next older occurrence.

```
begin interactive test "repeat search n"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  send "echo alpha1"
  expect "alpha1"
  expect "$ "
  send "echo beta"
  expect "beta"
  expect "$ "
  send "echo alpha2"
  expect "alpha2"
  expect "$ "
  sendraw 1b
  sleep 100ms
  sendraw 2f 61 6c 70 68 61 0a
  sleep 200ms
  sendraw 6e
  sleep 200ms
  sendraw 0a
  expect "alpha1"
  expect "$ "
  sendeof
  wait
end interactive test "repeat search n"
```

#### Test: repeat search opposite direction N

The `N` command repeats the most recent `/` or `?` search but reverses the direction. After `/` (backward) finds a match, `N` searches forward.

```
begin interactive test "repeat search opposite direction N"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  send "echo gamma1"
  expect "gamma1"
  expect "$ "
  send "echo gamma2"
  expect "gamma2"
  expect "$ "
  send "echo gamma3"
  expect "gamma3"
  expect "$ "
  sendraw 1b
  sleep 100ms
  sendraw 2f 67 61 6d 6d 61 0a
  sleep 200ms
  sendraw 6e
  sleep 200ms
  sendraw 4e
  sleep 200ms
  sendraw 0a
  expect "gamma"
  expect "$ "
  sendeof
  wait
end interactive test "repeat search opposite direction N"
```

#### Test: replace mode R

The `R` command enters insert mode in overwrite mode, replacing characters from the command line beginning at the current cursor position.

```
begin interactive test "replace mode R"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 62 63 64 65 66
  sendraw 1b
  sleep 100ms
  sendraw 30
  sleep 50ms
  sendraw 77
  sleep 50ms
  sendraw 52
  sleep 50ms
  sendraw 58 59
  sleep 50ms
  sendraw 1b
  sleep 100ms
  sendraw 0a
  expect "XYcdef"
  expect "$ "
  sendeof
  wait
end interactive test "replace mode R"
```

#### Test: comment out hash

The `#` command in vi command mode inserts `#` at the beginning of the current command line, treating it as a comment. The line is entered into the command history.

```
begin interactive test "comment out hash"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 68 65 6c 6c 6f
  sendraw 1b
  sleep 100ms
  sendraw 23
  sleep 200ms
  expect "$ "
  sendeof
  wait
end interactive test "comment out hash"
```

#### Test: delete to end of line D

The `D` command deletes all characters from the current cursor position to the end of the line. The deleted characters are placed in the save buffer.

```
begin interactive test "delete to end of line D"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 62 63 20 64 65 66
  sendraw 1b
  sleep 100ms
  sendraw 30
  sleep 50ms
  sendraw 77
  sleep 50ms
  sendraw 44
  sleep 100ms
  sendraw 0a
  expect "$ "
  sendeof
  wait
end interactive test "delete to end of line D"
```

#### Test: change to end of line C

The `C` command deletes from the current character to the end of the line and enters insert mode at the new end-of-line position.

```
begin interactive test "change to end of line C"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 62 63 20 64 65 66
  sendraw 1b
  sleep 100ms
  sendraw 30
  sleep 50ms
  sendraw 77
  sleep 50ms
  sendraw 43
  sleep 50ms
  sendraw 58 59 5a
  sleep 50ms
  sendraw 1b
  sleep 100ms
  sendraw 0a
  expect "XYZ"
  expect "$ "
  sendeof
  wait
end interactive test "change to end of line C"
```

#### Test: substitute entire line S

The `S` command clears the entire edit line and enters insert mode, allowing the user to type a completely new command.

```
begin interactive test "substitute entire line S"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 62 63
  sendraw 1b
  sleep 100ms
  sendraw 53
  sleep 50ms
  sendraw 65 63 68 6f 20 72 65 70 6c 61 63 65 64
  sleep 50ms
  sendraw 1b
  sleep 100ms
  sendraw 0a
  expect "replaced"
  expect "$ "
  sendeof
  wait
end interactive test "substitute entire line S"
```

#### Test: yank to end of line Y

The `Y` command yanks characters from the current cursor position to the end of the line into the save buffer. The current cursor position shall be unchanged.

```
begin interactive test "yank to end of line Y"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 62
  sendraw 1b
  sleep 100ms
  sendraw 59
  sleep 50ms
  sendraw 68
  sleep 50ms
  sendraw 50
  sleep 100ms
  sendraw 0a
  expect "bab"
  expect "$ "
  sendeof
  wait
end interactive test "yank to end of line Y"
```

#### Test: vi glob expand * with known files

In vi command mode, the `*` command performs pathname expansion on the current bigword and inserts all expansions separated by spaces. If the bigword has no glob characters, `*` is implicitly appended.

```
begin interactive test "vi glob expand * with known files"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  send "touch aaa_one aaa_two"
  expect "$ "
  # Type: echo aaa_<Esc>
  sendraw 65 63 68 6f 20 61 61 61 5f
  sendraw 1b
  sleep 100ms
  # Press * to glob-expand the bigword
  sendraw 2a
  sleep 200ms
  # Press Enter to execute
  sendraw 0a
  expect "aaa_one aaa_two"
  expect "$ "
  sendeof
  wait
end interactive test "vi glob expand * with known files"
```

#### Test: vi glob expand * on directory appends slash

If any directories are matched during `*` expansion, a `/` character shall be appended to the directory name. Known bash non-compliance: bash does not append the trailing slash.

```
begin interactive test "vi glob expand * on directory appends slash"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  send "mkdir testdir_comp"
  expect "$ "
  # Type: echo testdir<Esc>
  sendraw 65 63 68 6f 20 74 65 73 74 64 69 72
  sendraw 1b
  sleep 100ms
  # Press * to glob-expand
  sendraw 2a
  sleep 200ms
  sendraw 0a
  expect "testdir_comp/"
  expect "$ "
  sendeof
  wait
end interactive test "vi glob expand * on directory appends slash"
```

#### Test: vi filename completion unique file

The `\` command performs pathname expansion on the current bigword up to the largest uniquely matchable set. If a file is completely matched, a space is inserted after it.

```
begin interactive test "vi filename completion unique file"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  send "touch unique_file_xyz.txt"
  expect "$ "
  # Type: echo unique_f<Esc>
  sendraw 65 63 68 6f 20 75 6e 69 71 75 65 5f 66
  sendraw 1b
  sleep 100ms
  # Press \ to complete
  sendraw 5c
  sleep 200ms
  # Press Enter (we are in insert mode after completion)
  sendraw 0a
  expect "unique_file_xyz.txt"
  expect "$ "
  sendeof
  wait
end interactive test "vi filename completion unique file"
```

#### Test: vi filename completion unique directory adds slash

If the `\` completion uniquely matches a directory, a `/` character shall be inserted directly after the bigword.

```
begin interactive test "vi filename completion unique directory adds slash"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  send "mkdir only_dir_here"
  expect "$ "
  # Type: echo only_d<Esc>
  sendraw 65 63 68 6f 20 6f 6e 6c 79 5f 64
  sendraw 1b
  sleep 100ms
  # Press \ to complete
  sendraw 5c
  sleep 200ms
  # Press Enter
  sendraw 0a
  expect "only_dir_here"
  expect "$ "
  sendeof
  wait
end interactive test "vi filename completion unique directory adds slash"
```

#### Test: vi @ alias no alias enabled has no effect

The `@letter` command inserts the value of the alias named `_letter`. If no alias `_letter` is enabled, the command shall have no effect.

```
begin interactive test "vi @ alias no alias enabled has no effect"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  # Type: echo hello<Esc>
  sendraw 65 63 68 6f 20 68 65 6c 6c 6f
  sendraw 1b
  sleep 100ms
  # Press @x — no alias for 'x', should have no effect
  sendraw 40 78
  sleep 100ms
  # Press Enter — original line should be unchanged
  sendraw 0a
  expect "hello"
  expect "$ "
  sendeof
  wait
end interactive test "vi @ alias no alias enabled has no effect"
```

#### Test: vi v command invokes editor and executes result

The `v` command invokes the vi editor on the current command line in a temporary file. When the editor exits, the commands in the temporary file shall be executed and placed in the command history.

```
begin interactive test "vi v command invokes editor and executes result"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  # Create a fake vi that rewrites the temp file (avoids non-portable sed -i)
  send "printf '#!%s\nt=$(sed s/placeholder/from_editor/ \"$1\") && printf \"%%s\\n\" \"$t\" > \"$1\"\n' \"${SHELL%% *}\" > $HOME/vi && chmod +x $HOME/vi"
  expect "$ "
  send "export PATH=$HOME:$PATH"
  expect "$ "
  # Type: echo placeholder<Esc>
  sendraw 65 63 68 6f 20 70 6c 61 63 65 68 6f 6c 64 65 72
  sendraw 1b
  sleep 100ms
  # Press v to invoke the editor (vi from PATH)
  sendraw 76
  sleep 500ms
  expect timeout=3s "from_editor"
  expect "$ "
  sendeof
  wait
end interactive test "vi v command invokes editor and executes result"
```

#### Test: vi A appends at end of line

The `A` command enters insert mode after the end of the current command line, regardless of current cursor position.

```
begin interactive test "vi A appends at end of line"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 62
  sendraw 1b
  sleep 100ms
  sendraw 30
  sleep 50ms
  sendraw 41
  sleep 50ms
  sendraw 63 64
  sendraw 1b
  sleep 100ms
  sendraw 0a
  expect "abcd"
  expect "$ "
  sendeof
  wait
end interactive test "vi A appends at end of line"
```

#### Test: vi I inserts at beginning of line

The `I` command enters insert mode at the beginning of the current command line.

```
begin interactive test "vi I inserts at beginning of line"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 63 68 6f 20 61 62
  sendraw 1b
  sleep 100ms
  sendraw 49
  sleep 50ms
  sendraw 65
  sendraw 1b
  sleep 100ms
  sendraw 0a
  expect "ab"
  expect "$ "
  sendeof
  wait
end interactive test "vi I inserts at beginning of line"
```

#### Test: vi U undoes all changes

The `U` command undoes all changes made to the edit line, restoring it to its state before any modifications.

```
begin interactive test "vi U undoes all changes"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  send "echo baseline"
  expect "baseline"
  expect "$ "
  sendraw 1b
  sleep 100ms
  sendraw 6b
  sleep 100ms
  sendraw 24
  sleep 50ms
  sendraw 78
  sleep 50ms
  sendraw 78
  sleep 50ms
  sendraw 55
  sleep 100ms
  sendraw 0a
  expect "baseline"
  expect "$ "
  sendeof
  wait
end interactive test "vi U undoes all changes"
```

#### Test: vi caret moves to first non-blank

The `^` command moves the cursor to the first character on the input line that is not a blank.

```
begin interactive test "vi caret moves to first non-blank"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 62
  sendraw 1b
  sleep 100ms
  sendraw 5e
  sleep 50ms
  sendraw 72 5a
  sleep 100ms
  sendraw 0a
  expect "Zcho"
  expect "$ "
  sendeof
  wait
end interactive test "vi caret moves to first non-blank"
```

#### Test: vi dot repeat with count override

If the `.` command is preceded by a count, it shall override any count argument to the previous command. The overriding count becomes the count for subsequent `.` commands.

```
begin interactive test "vi dot repeat with count override"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 62 63 64 65 66
  sendraw 1b
  sleep 100ms
  sendraw 32 78
  sleep 100ms
  sendraw 31 2e
  sleep 100ms
  sendraw 0a
  expect "abc"
  expect "$ "
  sendeof
  wait
end interactive test "vi dot repeat with count override"
```

#### Test: vi r with count replaces multiple chars

The `r` command with a count prefix replaces the current and the following count-1 characters. The cursor is positioned on the last character changed.

```
begin interactive test "vi r with count replaces multiple chars"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 62 63 64
  sendraw 1b
  sleep 100ms
  sendraw 30 77
  sleep 50ms
  sendraw 33 72 5a
  sleep 100ms
  sendraw 0a
  expect "ZZZd"
  expect "$ "
  sendeof
  wait
end interactive test "vi r with count replaces multiple chars"
```

#### Test: vi insert mode Ctrl-W deletes word backward

The Ctrl-W character in insert mode deletes characters from before the cursor to the preceding word boundary.

```
begin interactive test "vi insert mode Ctrl-W deletes word backward"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 62 20 63 64
  sendraw 17
  sendraw 0a
  expect "ab"
  expect "$ "
  sendeof
  wait
end interactive test "vi insert mode Ctrl-W deletes word backward"
```

#### Test: vi insert mode SIGINT terminates editing

If sh receives a SIGINT signal in insert mode, it shall terminate command line editing on the current command line with the same effects as interrupting command mode.

```
begin interactive test "vi insert mode SIGINT terminates editing"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 70 61 72 74 69 61 6c
  sendraw 03
  expect "$ "
  send "echo after_insert_sigint"
  expect "after_insert_sigint"
  expect "$ "
  sendeof
  wait
end interactive test "vi insert mode SIGINT terminates editing"
```

#### Test: vi yy yanks entire line

If the motion command following `y` is `y` itself, the entire current command line shall be yanked into the save buffer. The cursor position is unchanged.

```
begin interactive test "vi yy yanks entire line"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 62
  sendraw 1b
  sleep 100ms
  sendraw 79 79
  sleep 50ms
  sendraw 24
  sleep 50ms
  sendraw 70
  sleep 100ms
  sendraw 0a
  expect "abecho ab"
  expect "$ "
  sendeof
  wait
end interactive test "vi yy yanks entire line"
```

#### Test: set +o vi disables vi mode

The command `set +o vi` shall disable vi-mode editing. After disabling, the shell reverts to its default line editing behavior.

```
begin interactive test "set +o vi disables vi mode"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  send "set +o vi"
  expect "$ "
  send "echo still_works"
  expect "still_works"
  expect "$ "
  sendeof
  wait
end interactive test "set +o vi disables vi mode"
```

#### Test: vi minus key navigates history backward

The `-` key in vi command mode shall behave identically to `k`, setting the current command line to the previous command in the history.

```
begin interactive test "vi minus key navigates history backward"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  send "echo minustest"
  expect "minustest"
  expect "$ "
  sendraw 1b
  sleep 100ms
  sendraw 2d
  sleep 100ms
  sendraw 0a
  expect "minustest"
  expect "$ "
  sendeof
  wait
end interactive test "vi minus key navigates history backward"
```

#### Test: vi plus key navigates history forward

The `+` key in vi command mode shall behave identically to `j`, setting the current command line to the next command in the history.

```
begin interactive test "vi plus key navigates history forward"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  send "echo plustest"
  expect "plustest"
  expect "$ "
  sendraw 1b
  sleep 100ms
  sendraw 6b
  sleep 100ms
  sendraw 2b
  sleep 100ms
  sendraw 0a
  expect "$ "
  sendeof
  wait
end interactive test "vi plus key navigates history forward"
```

#### Test: vi G without number goes to oldest history

The `G` command without a number shall set the current command line to the oldest command line stored in the shell command history.

```
begin interactive test "vi G without number goes to oldest history"
  spawn -i
  expect "$ "
  send "set -o vi"
  expect "$ "
  send "HISTSIZE=5"
  expect "$ "
  send "echo oldest_entry"
  expect "oldest_entry"
  expect "$ "
  send "echo newer_entry"
  expect "newer_entry"
  expect "$ "
  sendraw 1b
  sleep 100ms
  sendraw 47
  sleep 100ms
  sendraw 0a
  expect "$ "
  sendeof
  wait
end interactive test "vi G without number goes to oldest history"
```

#### Test: vi x deletes full multi-byte character

In vi command mode, `x` shall delete the character at the cursor. When the cursor is on a multi-byte character (like `é`), the entire character is deleted, not just one byte.

```
begin interactive test "vi x deletes full multi-byte character"
  spawn -i
  expect "$ "
  send "export LC_ALL=C.UTF-8"
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 c3 a9 62
  sleep 100ms
  sendraw 1b
  sleep 200ms
  sendraw 68
  sleep 200ms
  sendraw 78
  sleep 200ms
  sendraw 0a
  expect "ab"
  expect "$ "
  sendeof
  wait
end interactive test "vi x deletes full multi-byte character"
```

#### Test: vi dl deletes full multi-byte character

In vi command mode, `dl` (delete-motion-right) shall delete one character. When the cursor is on a multi-byte character, the entire character is deleted.

```
begin interactive test "vi dl deletes full multi-byte character"
  spawn -i
  expect "$ "
  send "export LC_ALL=C.UTF-8"
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 c3 a9 62
  sleep 100ms
  sendraw 1b
  sleep 200ms
  sendraw 68
  sleep 200ms
  sendraw 64 6c
  sleep 200ms
  sendraw 0a
  expect "ab"
  expect "$ "
  sendeof
  wait
end interactive test "vi dl deletes full multi-byte character"
```

#### Test: vi dw deletes word containing multi-byte characters

In vi command mode, `dw` shall delete from the cursor through the end of the current word. When the word contains multi-byte characters, they are all deleted correctly.

```
begin interactive test "vi dw deletes word containing multi-byte characters"
  spawn -i
  expect "$ "
  send "export LC_ALL=C.UTF-8"
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 c3 a9 c3 a8 20 62 62 62
  sleep 100ms
  sendraw 1b
  sleep 200ms
  sendraw 30
  sleep 200ms
  sendraw 64 77
  sleep 200ms
  sendraw 0a
  expect "bbb"
  expect "$ "
  sendeof
  wait
end interactive test "vi dw deletes word containing multi-byte characters"
```

#### Test: vi r replaces multi-byte character with ASCII

In vi command mode, `r` followed by a character shall replace the character at the cursor. When the cursor is on a multi-byte character, the entire character is replaced by the new character.

```
begin interactive test "vi r replaces multi-byte character with ASCII"
  spawn -i
  expect "$ "
  send "export LC_ALL=C.UTF-8"
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 c3 a9 62
  sleep 100ms
  sendraw 1b
  sleep 200ms
  sendraw 68
  sleep 200ms
  sendraw 72 58
  sleep 200ms
  sendraw 0a
  expect "aXb"
  expect "$ "
  sendeof
  wait
end interactive test "vi r replaces multi-byte character with ASCII"
```

#### Test: vi f finds multi-byte character

In vi command mode, `f` followed by a character shall move the cursor to the next occurrence of that character. When the target is a multi-byte character, the find matches the full character.

```
begin interactive test "vi f finds multi-byte character"
  spawn -i
  expect "$ "
  send "export LC_ALL=C.UTF-8"
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 62 c3 a9 63
  sleep 100ms
  sendraw 1b
  sleep 200ms
  sendraw 30
  sleep 200ms
  sendraw 66 c3 a9
  sleep 200ms
  sendraw 64 6c
  sleep 200ms
  sendraw 0a
  expect "abc"
  expect "$ "
  sendeof
  wait
end interactive test "vi f finds multi-byte character"
```

#### Test: vi a appends after multi-byte character

In vi command mode, `a` shall enter insert mode with the cursor positioned after the character at the current position. When the current character is multi-byte, the cursor advances past all its bytes.

```
begin interactive test "vi a appends after multi-byte character"
  spawn -i
  expect "$ "
  send "export LC_ALL=C.UTF-8"
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 c3 a9 62
  sleep 100ms
  sendraw 1b
  sleep 200ms
  sendraw 30
  sleep 200ms
  sendraw 66 c3 a9
  sleep 200ms
  sendraw 61 58
  sleep 200ms
  sendraw 1b
  sleep 200ms
  sendraw 30
  sleep 200ms
  sendraw 66 c3 a9
  sleep 200ms
  sendraw 64 6c
  sleep 200ms
  sendraw 0a
  expect "Xb"
  expect "$ "
  sendeof
  wait
end interactive test "vi a appends after multi-byte character"
```

#### Test: vi dollar moves to start of last multi-byte character

In vi command mode, `$` shall move the cursor to the last character on the line. When the last character is multi-byte, the cursor lands on its first byte, not a continuation byte.

```
begin interactive test "vi dollar moves to start of last multi-byte character"
  spawn -i
  expect "$ "
  send "export LC_ALL=C.UTF-8"
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 62 c3 a9
  sleep 100ms
  sendraw 1b
  sleep 200ms
  sendraw 30
  sleep 200ms
  sendraw 24
  sleep 200ms
  sendraw 78
  sleep 200ms
  sendraw 0a
  expect "ab"
  expect "$ "
  sendeof
  wait
end interactive test "vi dollar moves to start of last multi-byte character"
```

#### Test: vi p pastes after multi-byte character

In vi command mode, `p` shall insert the contents of the yank buffer after the current cursor position. When the cursor is on a multi-byte character, the paste position is after the full character.

```
begin interactive test "vi p pastes after multi-byte character"
  spawn -i
  expect "$ "
  send "export LC_ALL=C.UTF-8"
  expect "$ "
  send "set -o vi"
  expect "$ "
  sendraw 65 63 68 6f 20 61 c3 a9 62
  sleep 100ms
  sendraw 1b
  sleep 200ms
  sendraw 68
  sleep 200ms
  sendraw 78
  sleep 200ms
  sendraw 70
  sleep 200ms
  sendraw 24
  sleep 200ms
  sendraw 78
  sleep 200ms
  sendraw 0a
  expect "ab"
  expect "$ "
  sendeof
  wait
end interactive test "vi p pastes after multi-byte character"
```
