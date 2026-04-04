# Test Suite for 2.14 Pattern Matching Notation

This test suite covers **Section 2.14 Pattern Matching Notation** of the
POSIX.1-2024 Shell Command Language specification, including single-character
patterns, multi-character patterns, and filename expansion rules.

## Table of contents

- [2.14 Pattern Matching Notation](#214-pattern-matching-notation)
- [2.14.1 Patterns Matching a Single Character](#2141-patterns-matching-a-single-character)
- [2.14.2 Patterns Matching Multiple Characters](#2142-patterns-matching-multiple-characters)
- [2.14.3 Patterns Used for Filename Expansion](#2143-patterns-used-for-filename-expansion)

## 2.14 Pattern Matching Notation

The pattern matching notation described in this section is used to specify patterns for matching character strings in the shell. This notation is also used by some other utilities ([*find*](docs/posix/md/utilities/find.md), [*pax*](docs/posix/md/utilities/pax.md), and optionally [*make*](docs/posix/md/utilities/make.md)) and by some system interfaces ([*fnmatch*()](docs/posix/md/functions/fnmatch.md), [*glob*()](docs/posix/md/functions/glob.md), and [*wordexp*()](docs/posix/md/functions/wordexp.md)). Historically, pattern matching notation is related to, but slightly different from, the regular expression notation described in XBD [*9. Regular Expressions*](docs/posix/md/basedefs/V1_chap09.md#9-regular-expressions). For this reason, the description of the rules for this pattern matching notation are based on the description of regular expression notation, modified to account for the differences.

If an attempt is made to use pattern matching notation to match a string that contains one or more bytes that do not form part of a valid character, the behavior is unspecified. Since pathnames can contain such bytes, portable applications need to ensure that the current locale is the C or POSIX locale when performing pattern matching (or expansion) on arbitrary pathnames.

### Tests

Section 2.14 has no testable normative statements of its own — all
requirements are in the subsections below.

## 2.14.1 Patterns Matching a Single Character

The following patterns shall match a single character: ordinary characters, special pattern characters, and pattern bracket expressions. The pattern bracket expression also shall match a single collating element.

In a pattern, or part of one, where a shell-quoting `<backslash>` can be used, a `<backslash>` character shall escape the following character as described in [2.2.1 Escape Character (Backslash)](#221-escape-character-backslash), regardless of whether or not the `<backslash>` is inside a bracket expression. (The sequence `"\\"` represents one literal `<backslash>`.)

In a pattern, or part of one, where a shell-quoting `<backslash>` cannot be used to preserve the literal value of a character that would otherwise be treated as special:

- A `<backslash>` character that is not inside a bracket expression shall preserve the literal value of the following character, unless the following character is in a part of the pattern where shell quoting can be used and is a shell quoting character, in which case the behavior is unspecified.
- For the shell only, it is unspecified whether or not a `<backslash>` character inside a bracket expression preserves the literal value of the following character.

All of the requirements and effects of quoting on ordinary, shell special, and special pattern characters shall apply to escaping in this context, except where specified otherwise. (Situations where this applies include word expansions when a pattern used in pathname expansion is not present in the original word but results from an earlier expansion, or the argument to the [*find*](docs/posix/md/utilities/find.md) -*name* or -*path* primary as passed to [*find*](docs/posix/md/utilities/find.md), or the *pattern* argument to the [*fnmatch*()](docs/posix/md/functions/fnmatch.md) and [*glob*()](docs/posix/md/functions/glob.md) functions when FNM_NOESCAPE or GLOB_NOESCAPE is not set in *flags*, respectively.)

If a pattern ends with an unescaped `<backslash>`, the behavior is unspecified.

An ordinary character is a pattern that shall match itself. In a pattern, or part of one, where a shell-quoting `<backslash>` can be used, an ordinary character can be any character in the supported character set except for NUL, those special shell characters in [2.2 Quoting](#22-quoting) that require quoting, and the three special pattern characters described below. In a pattern, or part of one, where a shell-quoting `<backslash>` cannot be used to preserve the literal value of a character that would otherwise be treated as special, an ordinary character can be any character in the supported character set except for NUL and the three special pattern characters described below. Matching shall be based on the bit pattern used for encoding the character, not on the graphic representation of the character. If any character (ordinary, shell special, or pattern special) is quoted, or escaped with a `<backslash>`, that pattern shall match the character itself. The application shall ensure that it quotes or escapes any character that would otherwise be treated as special, in order for it to be matched as an ordinary character.

When unquoted, unescaped, and not inside a bracket expression, the following three characters shall have special meaning in the specification of patterns:

- `?`: A `<question-mark>` is a pattern that shall match any character.
- `*`: An `<asterisk>` is a pattern that shall match multiple characters, as described in [2.14.2 Patterns Matching Multiple Characters](#2142-patterns-matching-multiple-characters).
- `[`: A `<left-square-bracket>` shall introduce a bracket expression if the characters following it meet the requirements for bracket expressions stated in XBD [*9.3.5 RE Bracket Expression*](docs/posix/md/basedefs/V1_chap09.md#935-re-bracket-expression), except that the `<exclamation-mark>` character (`'!'`) shall replace the `<circumflex>` character (`'^'`) in its role in a non-matching list in the regular expression notation. A bracket expression starting with an unquoted `<circumflex>` character produces unspecified results. A `<left-square-bracket>` that does not introduce a valid bracket expression shall match the character itself.

### Tests

#### Test: ? matches single character in case

The `?` pattern matches exactly one character. An ordinary character
matches itself. Together, `a??le` matches a five-character string
starting with `a` and ending with `le`.

```
begin test "? matches single character in case"
  script
    case "apple" in
        a??le) echo "match" ;;
        *) echo "no" ;;
    esac
  expect
    stdout "match"
    stderr ""
    exit_code 0
end test "? matches single character in case"
```

#### Test: bracket expression matches one of the listed chars

A bracket expression `[abc]` matches exactly one of the listed
characters. An unmatched `[` that does not introduce a valid bracket
expression matches itself literally.

```
begin test "bracket expression matches one of the listed chars"
  script
    case "b" in
        [abc]) echo "match_bracket" ;;
    esac
    case "[" in
        [) echo "match_literal" ;;
    esac
  expect
    stdout "match_bracket\nmatch_literal"
    stderr ""
    exit_code 0
end test "bracket expression matches one of the listed chars"
```

#### Test: negated bracket expression with exclamation mark

In pattern bracket expressions, `!` replaces `^` for non-matching lists.
`[!a]` matches any single character that is not `a`.

```
begin test "negated bracket expression with exclamation mark"
  script
    case "b" in
        [!a]) echo "not_a" ;;
        *) echo "no" ;;
    esac
    case "a" in
        [!a]) echo "wrong" ;;
        *) echo "is_a" ;;
    esac
  expect
    stdout "not_a\nis_a"
    stderr ""
    exit_code 0
end test "negated bracket expression with exclamation mark"
```

#### Test: quoted and escaped * match literal asterisk

When a pattern special character is quoted or escaped with `\`, it matches
the character itself rather than having its special meaning.

```
begin test "quoted and escaped * match literal asterisk"
  script
    case "a*b" in
        "a*b") echo "quoted" ;;
    esac
    case "a*b" in
        a\*b) echo "escaped" ;;
    esac
  expect
    stdout "quoted\nescaped"
    stderr ""
    exit_code 0
end test "quoted and escaped * match literal asterisk"
```

#### Test: backslash escapes pattern character

In contexts where shell-quoting backslash can be used, `\` escapes the
following character, making it match literally.

```
begin test "backslash escapes pattern character"
  script
    case "?" in
        \?) echo "escaped_qmark" ;;
        *) echo "no" ;;
    esac
    case "[" in
        \[) echo "escaped_bracket" ;;
        *) echo "no" ;;
    esac
  expect
    stdout "escaped_qmark\nescaped_bracket"
    stderr ""
    exit_code 0
end test "backslash escapes pattern character"
```

## 2.14.2 Patterns Matching Multiple Characters

The following rules are used to construct patterns matching multiple characters from patterns matching a single character:

1. The `<asterisk>` (`'*'`) is a pattern that shall match any string, including the null string.
2. The concatenation of patterns matching a single character is a valid pattern that shall match the concatenation of the single characters or collating elements matched by each of the concatenated patterns.
3. The concatenation of one or more patterns matching a single character with one or more `<asterisk>` characters is a valid pattern. In such patterns, each `<asterisk>` shall match a string of zero or more characters, matching the greatest possible number of characters that still allows the remainder of the pattern to match the string.

### Tests

#### Test: * matches multiple characters and null string in case

The `*` pattern matches any string including the null string. `a*e`
matches `apple` (multiple chars), and `apple*` matches `apple` (null
string after the literal).

```
begin test "* matches multiple characters and null string in case"
  script
    case "apple" in
        a*e) echo "match_1" ;;
    esac
    case "apple" in
        apple*) echo "match_2" ;;
    esac
  expect
    stdout "match_1\nmatch_2"
    stderr ""
    exit_code 0
end test "* matches multiple characters and null string in case"
```

#### Test: concatenated patterns and greedy asterisk

Pattern concatenation: `x*c` is a valid pattern where `*` greedily
matches the longest string that still allows `c` to match the end.

```
begin test "concatenated patterns and greedy asterisk"
  script
    case "xyzabc" in
        x*c) echo yes ;;
        *) echo no ;;
    esac
  expect
    stdout "yes"
    stderr ""
    exit_code 0
end test "concatenated patterns and greedy asterisk"
```

#### Test: asterisk matches null string

The `*` pattern matches the null string. A case pattern of just `*`
matches any input, and `prefix*` matches the prefix with nothing after it.

```
begin test "asterisk matches null string"
  script
    case "" in
        *) echo "empty_match" ;;
    esac
    case "abc" in
        abc*) echo "null_suffix" ;;
    esac
  expect
    stdout "empty_match\nnull_suffix"
    stderr ""
    exit_code 0
end test "asterisk matches null string"
```

## 2.14.3 Patterns Used for Filename Expansion

The rules described so far in [2.14.1 Patterns Matching a Single Character](#2141-patterns-matching-a-single-character) and [2.14.2 Patterns Matching Multiple Characters](#2142-patterns-matching-multiple-characters) are qualified by the following rules that apply when pattern matching notation is used for filename expansion:

1. The `<slash>` character in a pathname shall be explicitly matched by using one or more `<slash>` characters in the pattern; it shall neither be matched by the `<asterisk>` or `<question-mark>` special characters nor by a bracket expression. `<slash>` characters in the pattern shall be identified before bracket expressions; thus, a `<slash>` cannot be included in a pattern bracket expression used for filename expansion. If a `<slash>` character is found following an unescaped `<left-square-bracket>` character before a corresponding `<right-square-bracket>` is found, the open bracket shall be treated as an ordinary character. For example, the pattern `"a[b/c]d"` does not match such pathnames as **abd** or **a/d**. It only matches a pathname of literally **a[b/c]d**.
2. If a filename begins with a `<period>` (`'.'`), the `<period>` shall be explicitly matched by using a `<period>` as the first character of the pattern or immediately following a `<slash>` character. The leading `<period>` shall not be matched by: It is unspecified whether an explicit `<period>` in a bracket expression matching list, such as `"[.abc]"`, can match a leading `<period>` in a filename.
    - The `<asterisk>` or `<question-mark>` special characters
    - A bracket expression containing a non-matching list, such as `"[!a]"`, a range expression, such as `"[%-0]"`, or a character class expression, such as `"[[:punct:]]"`
3. If a specified pattern contains any `'*'`, `'?'` or `'['` characters that will be treated as special (see [2.14.1 Patterns Matching a Single Character](#2141-patterns-matching-a-single-character)), it shall be matched against existing filenames and pathnames, as appropriate; if directory entries for dot and dot-dot exist, they may be ignored. Each component that contains any such characters shall require read permission in the directory containing that component. Each component that contains a `<backslash>` that will be treated as special may require read permission in the directory containing that component. Any component, except the last, that does not contain any `'*'`, `'?'` or `'['` characters that will be treated as special shall require search permission. If these permissions are denied, or if an attempt to open or search a pathname as a directory, or an attempt to read an opened directory, fails because of an error condition that is related to file system contents, this shall not be considered an error and pathname expansion shall continue as if the pathname had named an existing directory which had been successfully opened and read, or searched, and no matching directory entries had been found in it. For other error conditions it is unspecified whether pathname expansion fails or they are treated the same as when permission is denied. For example, given the pattern: search permission is needed for directories **/** and **foo**, search and read permissions are needed for directory **bar**, and search permission is needed for each **x*** directory. If the pattern matches any existing filenames or pathnames, the pattern shall be replaced with those filenames and pathnames, sorted according to the collating sequence in effect in the current locale. If this collating sequence does not have a total ordering of all characters (see XBD [*7.3.2 LC_COLLATE*](docs/posix/md/basedefs/V1_chap07.md#732-lccollate)), any filenames or pathnames that collate equally shall be further compared byte-by-byte using the collating sequence for the POSIX locale. If the pattern contains an open bracket (`'['`) that does not introduce a bracket expression as in XBD [*9.3.5 RE Bracket Expression*](docs/posix/md/basedefs/V1_chap09.md#935-re-bracket-expression), it is unspecified whether other unquoted `'*'`, `'?'`, `'['` or `<backslash>` characters within the same slash-delimited component of the pattern retain their special meanings or are treated as ordinary characters. For example, the pattern `"a*[/b*"` may match all filenames beginning with `'b'` in the directory `"a*["` or it may match all filenames beginning with `'b'` in all directories with names beginning with `'a'` and ending with `'['`. If the pattern does not match any existing filenames or pathnames, the pattern string shall be left unchanged.
  ```
  /foo/bar/x*/bam
  ```
  **Note:** A future version of this standard may require that directory entries for dot and dot-dot are ignored (if they exist) when matching patterns against existing filenames. For example, when expanding the pattern `".*"` the result would not include dot and dot-dot.
4. If a specified pattern does not contain any `'*'`, `'?'` or `'['` characters that will be treated as special, the pattern string shall be left unchanged.

### Tests

#### Test: * does not match across slash in pathname

The slash character must be explicitly matched in filename expansion;
`*` does not match across directory boundaries.

```
begin test "* does not match across slash in pathname"
  script
    mkdir -p tmp_pattern/dir
    touch tmp_pattern/dir/file.txt tmp_pattern/dir/file.md tmp_pattern/a.txt
    echo tmp_pattern/*/file.txt
  expect
    stdout "tmp_pattern/dir/file.txt"
    stderr ""
    exit_code 0
end test "* does not match across slash in pathname"
```

#### Test: * without slash does not match dir/file

A `*` in a single path component cannot cross a slash boundary. The
pattern `tmp_pattern/*file.txt` does not match `tmp_pattern/dir/file.txt`.

```
begin test "* without slash does not match dir/file"
  script
    mkdir -p tmp_pattern/dir
    touch tmp_pattern/dir/file.txt tmp_pattern/a.txt
    echo tmp_pattern/*file.txt
  expect
    stdout "tmp_pattern/\*file.txt"
    stderr ""
    exit_code 0
end test "* without slash does not match dir/file"
```

#### Test: glob * does not match dotfiles

A leading period in a filename must be explicitly matched; `*` does not
match files beginning with `.`.

```
begin test "glob * does not match dotfiles"
  script
    mkdir -p tmp_pattern
    touch tmp_pattern/.hidden tmp_pattern/a.txt tmp_pattern/dir
    echo tmp_pattern/*
  expect
    stdout "tmp_pattern/a\.txt tmp_pattern/dir"
    stderr ""
    exit_code 0
end test "glob * does not match dotfiles"
```

#### Test: .* matches hidden files

An explicit leading period (`.*`) matches dotfiles. Results are sorted
by the collating sequence.

```
begin test ".* matches hidden files"
  script
    mkdir -p tmp_pattern
    touch tmp_pattern/.hidden
    echo tmp_pattern/.*
  expect
    stdout ".*\.hidden.*"
    stderr ""
    exit_code 0
end test ".* matches hidden files"
```

#### Test: unmatched glob pattern left unchanged

When a pattern with special characters does not match any existing
filenames, the pattern string is left unchanged.

```
begin test "unmatched glob pattern left unchanged"
  script
    mkdir -p tmp_pattern
    echo tmp_pattern/*.md
  expect
    stdout "tmp_pattern/\*.md"
    stderr ""
    exit_code 0
end test "unmatched glob pattern left unchanged"
```

#### Test: pattern without special chars left unchanged

A pattern that contains no `*`, `?`, or `[` characters treated as special
is left unchanged regardless of whether matching files exist.

```
begin test "pattern without special chars left unchanged"
  script
    echo /no/such/path/here
  expect
    stdout "/no/such/path/here"
    stderr ""
    exit_code 0
end test "pattern without special chars left unchanged"
```

#### Test: glob results sorted by collating sequence

When a pattern matches multiple filenames, they are returned sorted
according to the collating sequence in effect.

```
begin test "glob results sorted by collating sequence"
  script
    mkdir -p tmp_pattern
    touch tmp_pattern/c.txt tmp_pattern/a.txt tmp_pattern/b.txt
    echo tmp_pattern/*.txt
  expect
    stdout "tmp_pattern/a\.txt tmp_pattern/b\.txt tmp_pattern/c\.txt"
    stderr ""
    exit_code 0
end test "glob results sorted by collating sequence"
```

#### Test: question mark does not match leading dot

The `?` special character does not match a leading period in a filename.

```
begin test "question mark does not match leading dot"
  script
    mkdir -p tmp_pattern
    touch tmp_pattern/.x tmp_pattern/ax
    echo tmp_pattern/?x
  expect
    stdout "tmp_pattern/ax"
    stderr ""
    exit_code 0
end test "question mark does not match leading dot"
```
