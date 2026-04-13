# Test Suite for Intrinsic Utility: fc

This test suite covers the **fc** intrinsic utility
as specified by POSIX.1-2024 (Section 1.7).

## Table of contents

- [utility: fc](#utility-fc)

## utility: fc

#### NAME

> fc — process the command history list

#### SYNOPSIS

> ```
> [UP]  fc [-r] [-e editor] [first
> [last]]
> fc -l [-nr] [first [last]]
> fc -s [old=new] [first]
> ```

#### DESCRIPTION

> The *fc* utility shall list, or shall edit and re-execute, commands previously entered to an interactive [*sh*](docs/posix/md/utilities/sh.md).
>
> The command history list shall reference commands by number. The first number in the list is selected arbitrarily. The relationship of a number to its command shall not change except when the user logs in and no other process is accessing the list, at which time the system may reset the numbering to start the oldest retained command at another number (usually 1). When the number reaches an implementation-defined upper limit, which shall be no smaller than the value in *HISTSIZE* or 32767 (whichever is greater), the shell may wrap the numbers, starting the next command with a lower number (usually 1). However, despite this optional wrapping of numbers, *fc* shall maintain the time-ordering sequence of the commands. For example, if four commands in sequence are given the numbers 32766, 32767, 1 (wrapped), and 2 as they are executed, command 32767 is considered the command previous to 1, even though its number is higher.
>
> When commands are edited (when the **-l** option is not specified), the resulting lines shall be entered at the end of the history list and then re-executed by [*sh*](docs/posix/md/utilities/sh.md). The *fc* command that caused the editing shall not be entered into the history list. If the editor returns a non-zero exit status, this shall suppress the entry into the history list and the command re-execution. Any command line variable assignments or redirection operators used with *fc* shall affect both the *fc* command itself as well as the command that results; for example:
>
> ```
> fc -s -- -1 2>/dev/null
> ```
>
> reinvokes the previous command, suppressing standard error for both *fc* and the previous command.

#### OPTIONS

> The *fc* utility shall conform to XBD [*12.2 Utility Syntax Guidelines*](docs/posix/md/basedefs/V1_chap12.md#122-utility-syntax-guidelines).
>
> The following options shall be supported:
>
> - **-e***editor*: Use the editor named by *editor* to edit the commands. The *editor* string is a utility name, subject to search via the *PATH* variable (see XBD [*8. Environment Variables*](docs/posix/md/basedefs/V1_chap08.md#8-environment-variables)). The value in the *FCEDIT* variable shall be used as a default when **-e** is not specified. If *FCEDIT* is null or unset, [*ed*](docs/posix/md/utilities/ed.md) shall be used as the editor.
> - **-l**: (The letter ell.) List the commands rather than invoking an editor on them. The commands shall be written in the sequence indicated by the *first* and *last* operands, as affected by **-r**, with each command preceded by the command number.
> - **-n**: Suppress command numbers when listing with **-l**.
> - **-r**: Reverse the order of the commands listed (with **-l**) or edited (with neither **-l** nor **-s**).
> - **-s**: Re-execute the command without invoking an editor.

#### OPERANDS

> The following operands shall be supported:
>
> - *first*, *last*: Select the commands to list or edit. The number of previous commands that can be accessed shall be determined by the value of the *HISTSIZE* variable. The value of *first* or *last* or both shall be one of the following:
>
>     - **[+]***number*: A positive number representing a command number; command numbers can be displayed with the **-l** option.
>     - **-***number*: A negative decimal number representing the command that was executed *number* of commands previously. For example, -1 is the immediately previous command.
>     - *string*: A string indicating the most recently entered command that begins with that string. If the *old*=*new* operand is not also specified with **-s**, the string form of the *first* operand cannot contain an embedded `<equals-sign>`.
>
>   When the synopsis form with **-s** is used:
>
>     - If *first* is omitted, the previous command shall be used.
>
>   For the synopsis forms without **-s**:
>
>     - If *last* is omitted, *last* shall default to the previous command when **-l** is specified; otherwise, it shall default to *first*.
>     - If *first* and *last* are both omitted, the previous 16 commands shall be listed or the previous single command shall be edited (based on the **-l** option).
>     - If *first* and *last* are both present, all of the commands from *first* to *last* shall be edited (without **-l**) or listed (with **-l**). Editing multiple commands shall be accomplished by presenting to the editor all of the commands at one time, each command starting on a new line. If *first* represents a newer command than *last*, the commands shall be listed or edited in reverse sequence, equivalent to using **-r**. For example, the following commands on the first line are equivalent to the corresponding commands on the second:
>       ```
>       fc -r 10 20    fc    30 40
>       fc    20 10    fc -r 40 30
>       ```
>     - When a range of commands is used, it shall not be an error to specify *first* or *last* values that are not in the history list; *fc* shall substitute the value representing the oldest or newest command in the list, as appropriate. For example, if there are only ten commands in the history list, numbered 1 to 10: shall list and edit, respectively, all ten commands.
>       ```
>       fc -l
>       fc 1 99
>       ```
> - *old*=*new*: Replace the first occurrence of string *old* in the commands to be re-executed by the string *new*.

#### STDIN

> Not used.

#### INPUT FILES

> None.

#### ENVIRONMENT VARIABLES

> The following environment variables shall affect the execution of *fc*:
>
> - *FCEDIT*: This variable, when expanded by the shell, shall determine the default value for the **-e** *editor* option's *editor* option-argument. If *FCEDIT* is null or unset, [*ed*](docs/posix/md/utilities/ed.md) shall be used as the editor.
> - *HISTFILE*: Determine a pathname naming a command history file. If the *HISTFILE* variable is not set, the shell may attempt to access or create a file **.sh_history** in the directory referred to by the *HOME* environment variable. If the shell cannot obtain both read and write access to, or create, the history file, it shall use an unspecified mechanism that allows the history to operate properly. (References to history "file" in this section shall be understood to mean this unspecified mechanism in such cases.) An implementation may choose to access this variable only when initializing the history file; this initialization shall occur when *fc* or [*sh*](docs/posix/md/utilities/sh.md) first attempt to retrieve entries from, or add entries to, the file, as the result of commands issued by the user, the file named by the *ENV* variable, or implementation-defined system start-up files. In some historical shells, the history file is initialized just after the *ENV* file has been processed. Therefore, it is implementation-defined whether changes made to *HISTFILE* after the history file has been initialized are effective. Implementations may choose to disable the history list mechanism for users with appropriate privileges who do not set *HISTFILE ;* the specific circumstances under which this occurs are implementation-defined. If more than one instance of the shell is using the same history file, it is unspecified how updates to the history file from those shells interact. As entries are deleted from the history file, they shall be deleted oldest first. It is unspecified when history file entries are physically removed from the history file.
> - *HISTSIZE*: Determine a decimal number representing the limit to the number of previous commands that are accessible. If this variable is unset, an unspecified default greater than or equal to 128 shall be used. The maximum number of commands in the history list is unspecified, but shall be at least 128. An implementation may choose to access this variable only when initializing the history file, as described under *HISTFILE .* Therefore, it is unspecified whether changes made to *HISTSIZE* after the history file has been initialized are effective.
> - *LANG*: Provide a default value for the internationalization variables that are unset or null. (See XBD [*8.2 Internationalization Variables*](docs/posix/md/basedefs/V1_chap08.md#82-internationalization-variables) for the precedence of internationalization variables used to determine the values of locale categories.)
> - *LC_ALL*: If set to a non-empty string value, override the values of all the other internationalization variables.
> - *LC_CTYPE*: Determine the locale for the interpretation of sequences of bytes of text data as characters (for example, single-byte as opposed to multi-byte characters in arguments and input files).
> - *LC_MESSAGES*: Determine the locale that should be used to affect the format and contents of diagnostic messages written to standard error.
> - *NLSPATH*: Determine the location of messages objects and message catalogs.

#### ASYNCHRONOUS EVENTS

> Default.

#### STDOUT

> When the **-l** option is used to list commands, the format of each command in the list shall be as follows:
>
> ```
> "%d\t%s\n", <line number>, <command>
> ```
>
> If both the **-l** and **-n** options are specified, the format of each command shall be:
>
> ```
> "\t%s\n", <command>
> ```
>
> If the \<*command*\> consists of more than one line, the lines after the first shall be displayed as:
>
> ```
> "\t%s\n", <continued-command>
> ```

#### STDERR

> The standard error shall be used only for diagnostic messages.

#### OUTPUT FILES

> None.

#### EXTENDED DESCRIPTION

> None.

#### EXIT STATUS

> The following exit values shall be returned:
>
> - 0: Successful completion of the listing.
> - \>0: An error occurred.
>
> Otherwise, the exit status shall be that of the commands executed by *fc*.

#### CONSEQUENCES OF ERRORS

> Default.

---

*The following sections are informative.*

#### APPLICATION USAGE

> This utility is required to be intrinsic. See [*1.7 Intrinsic Utilities*](docs/posix/md/utilities/V3_chap01.md#17-intrinsic-utilities) for details.
>
> Since editors sometimes use file descriptors as integral parts of their editing, redirecting their file descriptors as part of the *fc* command can produce unexpected results. For example, if [*vi*](docs/posix/md/utilities/vi.md) is the *FCEDIT* editor, the command:
>
> ```
> fc -s | more
> ```
>
> does not work correctly on many systems.
>
> Users on windowing systems may want to have separate history files for each window by setting *HISTFILE* as follows:
>
> ```
> HISTFILE=$HOME/.sh_hist$$
> ```

#### EXAMPLES

> None.

#### RATIONALE

> This utility is based on the *fc* built-in of the KornShell.
>
> An early proposal specified the **-e** option as **[-e** *editor* **[***old*= *new* **]]**, which is not historical practice. Historical practice in *fc* of either **[-e** *editor***]** or **[-e - [** *old*= *new* **]]** is acceptable, but not both together. To clarify this, a new option **-s** was introduced replacing the **[-e -]**. This resolves the conflict and makes *fc* conform to the Utility Syntax Guidelines.
>
> - *HISTFILE*: Some implementations of the KornShell check for the superuser and do not create a history file unless *HISTFILE* is set. This is done primarily to avoid creating unlinked files in the root file system when logging in during single-user mode. *HISTFILE* must be set for the superuser to have history.
> - *HISTSIZE*: Needed to limit the size of history files. It is the intent of the standard developers that when two shells share the same history file, commands that are entered in one shell shall be accessible by the other shell. Because of the difficulties of synchronization over a network, the exact nature of the interaction is unspecified.
>
> The initialization process for the history file can be dependent on the system start-up files, in that they may contain commands that effectively preempt the settings the user has for *HISTFILE* and *HISTSIZE .* For example, function definition commands are recorded in the history file. If the system administrator includes function definitions in some system start-up file called before the *ENV* file, the history file is initialized before the user can influence its characteristics. In some historical shells, the history file is initialized just after the *ENV* file has been processed. Because of these situations, the text requires the initialization process to be implementation-defined.
>
> Consideration was given to omitting the *fc* utility in favor of the command line editing feature in [*sh*](docs/posix/md/utilities/sh.md). For example, in [*vi*](docs/posix/md/utilities/vi.md) editing mode, typing `"<ESC> v"` is equivalent to:
>
> ```
> EDITOR=vi fc
> ```
>
> However, the *fc* utility allows the user the flexibility to edit multiple commands simultaneously (such as *fc* 10 20) and to use editors other than those supported by [*sh*](docs/posix/md/utilities/sh.md) for command line editing.
>
> In the KornShell, the alias **r** ("re-do") is preset to *fc* **-e -** (equivalent to the POSIX *fc* **-s**). This is probably an easier command name to remember than *fc* ("fix command"), but it does not meet the Utility Syntax Guidelines. Renaming *fc* to *hist* or *redo* was considered, but since this description closely matches historical KornShell practice already, such a renaming was seen as gratuitous. Users are free to create aliases whenever odd historical names such as *fc*, [*awk*](docs/posix/md/utilities/awk.md), [*cat*](docs/posix/md/utilities/cat.md), [*grep*](docs/posix/md/utilities/grep.md), or [*yacc*](docs/posix/md/utilities/yacc.md) are standardized by POSIX.
>
> Command numbers have no ordering effects; they are like serial numbers. The **-r** option and -*number* operand address the sequence of command execution, regardless of serial numbers. So, for example, if the command number wrapped back to 1 at some arbitrary point, there would be no ambiguity associated with traversing the wrap point. For example, if the command history were:
>
> ```
> 32766: echo 1
> 32767: echo 2
> 1: echo 3
> ```
>
> the number -2 refers to command 32767 because it is the second previous command, regardless of serial number.

#### FUTURE DIRECTIONS

> None.

#### SEE ALSO

> [*sh*](docs/posix/md/utilities/sh.md)
>
> XBD [*8. Environment Variables*](docs/posix/md/basedefs/V1_chap08.md#8-environment-variables), [*12.2 Utility Syntax Guidelines*](docs/posix/md/basedefs/V1_chap12.md#122-utility-syntax-guidelines)

#### CHANGE HISTORY

> First released in Issue 4.

#### Issue 5

> The FUTURE DIRECTIONS section is added.

#### Issue 6

> This utility is marked as part of the User Portability Utilities option.
>
> In the ENVIRONMENT VARIABLES section, the text "user's home directory" is updated to "directory referred to by the *HOME* environment variable".

#### Issue 7

> SD5-XCU-ERN-97 is applied, updating the SYNOPSIS.

#### Issue 8

> Austin Group Defect 854 is applied, adding a note to the APPLICATION USAGE section that this utility is required to be intrinsic.
>
> Austin Group Defect 1122 is applied, changing the description of *NLSPATH .*

*End of informative text.*

### Tests

#### Test: fc -l lists command history

Verifies that `fc -l` with no range arguments lists previously entered commands from the history, including their command text.

```
begin interactive test "fc -l lists command history"
  spawn -i
  expect "$ "
  send "echo fc_test_1"
  expect "fc_test_1"
  expect "$ "
  send "echo fc_test_2"
  expect "fc_test_2"
  expect "$ "
  send "fc -l"
  expect "echo fc_test_1"
  expect "echo fc_test_2"
  sendeof
  wait
end interactive test "fc -l lists command history"
```

#### Test: fc -ln suppresses line numbers

Verifies that combining the `-l` and `-n` options causes `fc` to list commands without preceding line numbers, as POSIX requires.

```
begin interactive test "fc -ln suppresses line numbers"
  spawn -i
  expect "$ "
  send "echo ln_test"
  expect "ln_test"
  expect "$ "
  send "fc -ln"
  expect "echo ln_test"
  sendeof
  wait
end interactive test "fc -ln suppresses line numbers"
```

#### Test: fc -s re-executes previous command

Verifies that `fc -s` with no operands re-executes the most recently entered command without invoking an editor.

```
begin interactive test "fc -s re-executes previous command"
  spawn -i
  expect "$ "
  send "echo reexec_target"
  expect "reexec_target"
  expect "$ "
  send "fc -s"
  expect "reexec_target"
  sendeof
  wait
end interactive test "fc -s re-executes previous command"
```

#### Test: fc -s old=new substitution

Verifies that `fc -s old=new` replaces the first occurrence of the string "old" with "new" in the previous command before re-executing it.

```
begin interactive test "fc -s old=new substitution"
  spawn -i
  expect "$ "
  send "echo old_value"
  expect "old_value"
  expect "$ "
  send "fc -s old_value=new_value"
  expect "new_value"
  sendeof
  wait
end interactive test "fc -s old=new substitution"
```

#### Test: fc -l with first/last range

Verifies that `fc -l first last` lists only the commands within the specified range of negative offsets, selecting a subset of the history.

```
begin interactive test "fc -l with first/last range"
  spawn -i
  expect "$ "
  send "echo range_a"
  expect "range_a"
  expect "$ "
  send "echo range_b"
  expect "range_b"
  expect "$ "
  send "echo range_c"
  expect "range_c"
  expect "$ "
  send "fc -l -2 -1"
  expect "echo range_b"
  expect "echo range_c"
  sendeof
  wait
end interactive test "fc -l with first/last range"
```

#### Test: fc with FCEDIT editor invocation

Verifies that when `fc` is invoked without `-l` or `-s`, it launches the editor specified by the FCEDIT environment variable, and re-executes the edited command.

```
begin interactive test "fc with FCEDIT editor invocation"
  spawn -i
  expect "$ "
  send "printf '#!/bin/sh\nf=$1; sed s/original_cmd/edited_cmd/ <$f >/tmp/_fc_tmp && mv /tmp/_fc_tmp $f\n' >/tmp/_fc_ed.sh; chmod +x /tmp/_fc_ed.sh; export FCEDIT=/tmp/_fc_ed.sh"
  expect "$ "
  send "echo original_cmd"
  expect "original_cmd"
  expect "$ "
  send "fc"
  expect "edited_cmd"
  expect "$ "
  sendeof
  wait
end interactive test "fc with FCEDIT editor invocation"
```

#### Test: fc -e editor option

Verifies that `fc -e editor` uses the specified editor program to edit the previous command, and then re-executes the modified command.

```
begin interactive test "fc -e editor option"
  spawn -i
  expect "$ "
  send "printf '#!/bin/sh\nf=$1; sed s/fe_original/fe_edited/ <$f >/tmp/_fc_tmp2 && mv /tmp/_fc_tmp2 $f\n' >/tmp/_fc_ed2.sh; chmod +x /tmp/_fc_ed2.sh"
  expect "$ "
  send "echo fe_original"
  expect "fe_original"
  expect "$ "
  send "fc -e /tmp/_fc_ed2.sh"
  expect "fe_edited"
  expect "$ "
  send "true"
  expect "$ "
  sendeof
  wait
end interactive test "fc -e editor option"
```

#### Test: fc -e non-zero exit suppresses re-execution

Verifies that if the editor invoked by `fc -e` exits with a non-zero status, the command is not re-executed and is not entered into the history list.

```
begin interactive test "fc -e non-zero exit suppresses re-execution"
  spawn -i
  expect "$ "
  send "echo should_not_reexec"
  expect "should_not_reexec"
  expect "$ "
  send "fc -e false; true"
  expect "$ "
  sendeof
  wait
end interactive test "fc -e non-zero exit suppresses re-execution"
```

#### Test: fc -l reverse order

Verifies that when the first operand represents a newer command than the last operand (e.g., `fc -l -1 -2`), the commands are listed in reverse chronological order.

```
begin interactive test "fc -l reverse order"
  spawn -i
  expect "$ "
  send "echo rev_first"
  expect "rev_first"
  expect "$ "
  send "echo rev_second"
  expect "rev_second"
  expect "$ "
  send "fc -l -1 -2"
  expect "rev_second"
  expect "rev_first"
  sendeof
  wait
end interactive test "fc -l reverse order"
```

#### Test: HISTSIZE controls accessible history

Verifies that setting HISTSIZE limits the number of commands retained in the history list. Commands older than the HISTSIZE window should no longer appear in `fc -l` output.

```
begin interactive test "HISTSIZE controls accessible history"
  spawn -i
  expect "$ "
  send "HISTSIZE=3"
  expect "$ "
  send "echo old_gone"
  expect "$ "
  send "echo keep1"
  expect "$ "
  send "echo keep2"
  expect "$ "
  send "echo keep3"
  expect "$ "
  send "fc -l | grep -c old_gone || true; echo end_fc_check"
  expect "0"
  expect "end_fc_check"
  sendeof
  wait
end interactive test "HISTSIZE controls accessible history"
```

#### Test: fc -l with absolute line numbers

Verifies that `fc -l` accepts positive (absolute) command numbers as the first and last operands, listing all commands in that numeric range.

```
begin interactive test "fc -l with absolute line numbers"
  spawn -i
  expect "$ "
  send "echo a"
  expect "a"
  expect "$ "
  send "echo b"
  expect "b"
  expect "$ "
  send "echo c"
  expect "c"
  expect "$ "
  send "fc -l 1 3"
  expect "echo a"
  expect "echo b"
  expect "echo c"
  sendeof
  wait
end interactive test "fc -l with absolute line numbers"
```

#### Test: fc -l with out-of-range line number

Verifies that specifying a line number far beyond the history does not produce an error. POSIX requires `fc` to substitute the oldest or newest command as appropriate when the operand is out of range.

```
begin interactive test "fc -l with out-of-range line number"
  spawn -i
  expect "$ "
  send "fc -l 9999; echo fc_done"
  expect "fc_done"
  sendeof
  wait
end interactive test "fc -l with out-of-range line number"
```

#### Test: fc -s old=new with string-pattern operand

Verifies that `fc -s old=new first` performs text substitution and re-execution on a command selected by a string prefix operand, combining both features in a single invocation.

```
begin interactive test "fc -s old=new with string-pattern operand"
  spawn -i
  expect "$ "
  send "echo OLD_VALUE"
  expect "OLD_VALUE"
  expect "$ "
  send "fc -s OLD=NEW echo"
  expect "NEW_VALUE"
  sendeof
  wait
end interactive test "fc -s old=new with string-pattern operand"
```

#### Test: fc -lr lists in reverse order

Verifies that `fc -lr` (combined list and reverse options) lists the specified range of history commands in reverse chronological order.

```
begin interactive test "fc -lr lists in reverse order"
  spawn -i
  expect "$ "
  send "echo rev_alpha"
  expect "rev_alpha"
  expect "$ "
  send "echo rev_beta"
  expect "rev_beta"
  expect "$ "
  send "echo rev_gamma"
  expect "rev_gamma"
  expect "$ "
  send "fc -lr -3 -1"
  expect "rev_gamma"
  expect "rev_beta"
  expect "rev_alpha"
  sendeof
  wait
end interactive test "fc -lr lists in reverse order"
```

#### Test: fc -l -r with explicit range reverses listing

Verifies that `fc -l -r` with an explicit first/last range reverses the display order of the selected commands, listing the most recent one first.

```
begin interactive test "fc -l -r with explicit range reverses listing"
  spawn -i
  expect "$ "
  send "echo rl_one"
  expect "rl_one"
  expect "$ "
  send "echo rl_two"
  expect "rl_two"
  expect "$ "
  send "fc -l -r -2 -1"
  expect "rl_two"
  expect "rl_one"
  sendeof
  wait
end interactive test "fc -l -r with explicit range reverses listing"
```

#### Test: fc -l -1 lists only the most recent command

Verifies that `fc -l -1 -1` selects exactly one command (the most recently entered) and lists it, confirming negative-offset operands correctly identify single entries.

```
begin interactive test "fc -l -1 lists only the most recent command"
  spawn -i
  expect "$ "
  send "echo neg_latest"
  expect "neg_latest"
  expect "$ "
  send "fc -l -1 -1"
  expect "echo neg_latest"
  sendeof
  wait
end interactive test "fc -l -1 lists only the most recent command"
```

#### Test: fc -l -3 -2 lists a range by negative offsets

Verifies that `fc -l -3 -2` lists the two commands that are three and two positions back in the history, confirming negative-offset ranges work correctly.

```
begin interactive test "fc -l -3 -2 lists a range by negative offsets"
  spawn -i
  expect "$ "
  send "echo neg_a"
  expect "neg_a"
  expect "$ "
  send "echo neg_b"
  expect "neg_b"
  expect "$ "
  send "echo neg_c"
  expect "neg_c"
  expect "$ "
  send "fc -l -3 -2"
  expect "echo neg_a"
  expect "echo neg_b"
  sendeof
  wait
end interactive test "fc -l -3 -2 lists a range by negative offsets"
```

#### Test: fc -s with negative number re-executes offset command

Verifies that `fc -s -N` re-executes the command N positions back in the history, rather than always re-executing the immediately previous command.

```
begin interactive test "fc -s with negative number re-executes offset command"
  spawn -i
  expect "$ "
  send "echo neg_reexec_target"
  expect "neg_reexec_target"
  expect "$ "
  send "echo filler"
  expect "filler"
  expect "$ "
  send "fc -s -2"
  expect "neg_reexec_target"
  sendeof
  wait
end interactive test "fc -s with negative number re-executes offset command"
```

#### Test: fc -l with first > last lists in reverse sequence

Verifies that when the first operand refers to a newer command than the last operand, `fc -l` automatically lists the commands in reverse sequence, as POSIX specifies.

```
begin interactive test "fc -l with first > last lists in reverse sequence"
  spawn -i
  expect "$ "
  send "echo rng_first"
  expect "rng_first"
  expect "$ "
  send "echo rng_second"
  expect "rng_second"
  expect "$ "
  send "echo rng_third"
  expect "rng_third"
  expect "$ "
  send "fc -l -1 -3"
  expect "rng_third"
  expect "rng_second"
  expect "rng_first"
  sendeof
  wait
end interactive test "fc -l with first > last lists in reverse sequence"
```

#### Test: fc -l output contains command text

Verifies that the output of `fc -l` includes the full command text for each listed entry, matching the POSIX format of a line number followed by the command.

```
begin interactive test "fc -l output contains command text"
  spawn -i
  expect "$ "
  send "echo numfmt_test"
  expect "numfmt_test"
  expect "$ "
  send "fc -l -1 -1"
  expect "echo numfmt_test"
  sendeof
  wait
end interactive test "fc -l output contains command text"
```

#### Test: fc -l line numbers are monotonically increasing

Verifies that when listing a range of commands with `fc -l`, the entries appear in chronological order, confirming that command numbers maintain a time-ordered sequence.

```
begin interactive test "fc -l line numbers are monotonically increasing"
  spawn -i
  expect "$ "
  send "echo mono_a"
  expect "mono_a"
  expect "$ "
  send "echo mono_b"
  expect "mono_b"
  expect "$ "
  send "echo mono_c"
  expect "mono_c"
  expect "$ "
  send "fc -l -3 -1"
  expect "echo mono_a"
  expect "echo mono_b"
  expect "echo mono_c"
  sendeof
  wait
end interactive test "fc -l line numbers are monotonically increasing"
```

#### Test: fc -l shows recently entered command

Verifies that a command entered immediately before `fc -l` appears in the listing, confirming that commands are added to the history before `fc` reads it.

```
begin interactive test "fc -l shows recently entered command"
  spawn -i
  expect "$ "
  send "echo hs_test_cmd"
  expect "hs_test_cmd"
  expect "$ "
  send "fc -l"
  expect "hs_test_cmd"
  sendeof
  wait
end interactive test "fc -l shows recently entered command"
```

#### Test: fc -l with no operands lists recent commands

Verifies that `fc -l` with no first/last operands defaults to listing the previous 16 commands (or fewer if the history is shorter), as POSIX specifies.

```
begin interactive test "fc -l with no operands lists recent commands"
  spawn -i
  expect "$ "
  send "echo bare_list_test"
  expect "bare_list_test"
  expect "$ "
  send "fc -l"
  expect "echo bare_list_test"
  sendeof
  wait
end interactive test "fc -l with no operands lists recent commands"
```

#### Test: fc -s re-executes and enters result into history

Verifies that a command re-executed via `fc -s` is entered into the history list, so it can subsequently be retrieved by `fc -l`.

```
begin interactive test "fc -s re-executes and enters result into history"
  spawn -i
  expect "$ "
  send "echo reenter_hist"
  expect "reenter_hist"
  expect "$ "
  send "fc -s"
  expect "reenter_hist"
  expect "$ "
  send "fc -l -1 -1"
  expect "echo reenter_hist"
  sendeof
  wait
end interactive test "fc -s re-executes and enters result into history"
```

#### Test: fc produces no stderr on valid usage

Verifies that `fc -l` does not write anything to standard error when invoked with valid arguments. POSIX requires that stderr is used only for diagnostic messages.

```
begin interactive test "fc produces no stderr on valid usage"
  spawn -i
  expect "$ "
  send "HISTSIZE=100"
  expect "$ "
  send "echo fc_test"
  expect "fc_test"
  expect "$ "
  send "fc -l -1 -1 2>/tmp/fc_stderr_test"
  expect "echo fc_test"
  expect "$ "
  send "[ ! -s /tmp/fc_stderr_test ] && echo fc_stderr_empty || echo fc_stderr_nonempty; rm -f /tmp/fc_stderr_test"
  expect "fc_stderr_empty"
  expect "$ "
  sendeof
  wait
end interactive test "fc produces no stderr on valid usage"
```
