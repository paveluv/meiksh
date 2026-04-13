# Test Suite for 1.1.2.1 Arithmetic Precision and Operations

This test suite covers XCU Section 1.1.2.1 (Arithmetic Precision and Operations)
from POSIX.1-2024. This section defines the arithmetic semantics used by the
shell's `$((…))` arithmetic expansion (Section 2.6.4), which references it
directly. Section 2.6.4 narrows the scope: only signed long integer arithmetic
is required, `sizeof()` and prefix/postfix `++`/`--` are not required, and
selection/iteration/jump statements are not supported.

## Table of contents

- [1.1.2.1 Arithmetic Precision and Operations](#1121-arithmetic-precision-and-operations)

## 1.1.2.1 Arithmetic Precision and Operations

Integer variables and constants, including the values of operands and option-arguments, used by the standard utilities listed in this volume of POSIX.1-2024 shall be implemented as equivalent to the ISO C standard **signed long** data type; floating point shall be implemented as equivalent to the ISO C standard **double** type. Conversions between types shall be as described in the ISO C standard. All variables shall be initialized to zero if they are not otherwise assigned by the input to the application.

Arithmetic operators and control flow keywords shall be implemented as equivalent to those in the cited ISO C standard section, as listed in [Selected ISO C Standard Operators and Control Flow Keywords](#tagtcjh_10).

**Note:** The comma operator (section 6.5.17 of the ISO C standard) is intentionally not included in the table. It need not be supported by implementations.

Table: Selected ISO C Standard Operators and Control Flow Keywords

| **Operation** | **ISO C Standard Equivalent Reference** |
| --- | --- |
| () | Section 6.5.1, Primary Expressions |
| postfix ++ postfix -- | Section 6.5.2, Postfix Operators |
| unary + unary - prefix ++ prefix -- ~ ! *sizeof*() | Section 6.5.3, Unary Operators |
| * / % | Section 6.5.5, Multiplicative Operators |
| + - | Section 6.5.6, Additive Operators |
| \<\< \>\> | Section 6.5.7, Bitwise Shift Operators |
| \<, \<= \>, \>= | Section 6.5.8, Relational Operators |
| == != | Section 6.5.9, Equality Operators |
| & | Section 6.5.10, Bitwise AND Operator |
| ^ | Section 6.5.11, Bitwise Exclusive OR Operator |
| \| | Section 6.5.12, Bitwise Inclusive OR Operator |
| && | Section 6.5.13, Logical AND Operator |
| \|\| | Section 6.5.14, Logical OR Operator |
| *expr*?*expr*:*expr* | Section 6.5.15, Conditional Operator |
| =, *=, /=, %=, +=, -= \<\<=, \>\>=, &=, ^=, \|= | Section 6.5.16, Assignment Operators |
| **if** () **if** () ... **else** **switch** () | Section 6.8.4, Selection Statements |
| **while** () **do** ... **while** () **for** () | Section 6.8.5, Iteration Statements |
| **goto** **continue** **break** **return** | Section 6.8.6, Jump Statements |

The evaluation of arithmetic expressions shall be equivalent to that described in Section 6.5, Expressions, of the ISO C standard.

### Tests

#### Test: parenthesized grouping

Parentheses override the default operator precedence. Without parentheses,
`2 + 3 * 4` evaluates as `2 + 12 = 14`; with parentheses around the
addition, it evaluates as `5 * 4 = 20`.

```
begin test "parenthesized grouping"
  script
    echo $(( (2 + 3) * 4 ))
  expect
    stdout "20"
    stderr ""
    exit_code 0
end test "parenthesized grouping"
```

#### Test: multiplication

The `*` operator performs integer multiplication.

```
begin test "multiplication"
  script
    echo $(( 6 * 7 ))
  expect
    stdout "42"
    stderr ""
    exit_code 0
end test "multiplication"
```

#### Test: division truncates toward zero

Integer division truncates the fractional part. The result of `7 / 2`
shall be `3`, not `3.5` or `4`.

```
begin test "division truncates toward zero"
  script
    echo $(( 7 / 2 ))
  expect
    stdout "3"
    stderr ""
    exit_code 0
end test "division truncates toward zero"
```

#### Test: negative division truncates toward zero

For negative operands, integer division shall truncate toward zero per
ISO C semantics. `-7 / 2` shall be `-3`.

```
begin test "negative division truncates toward zero"
  script
    echo $(( -7 / 2 ))
  expect
    stdout "-3"
    stderr ""
    exit_code 0
end test "negative division truncates toward zero"
```

#### Test: modulo operator

The `%` operator yields the remainder of integer division. `17 % 5`
shall be `2`.

```
begin test "modulo operator"
  script
    echo $(( 17 % 5 ))
  expect
    stdout "2"
    stderr ""
    exit_code 0
end test "modulo operator"
```

#### Test: negative modulo sign follows dividend

Per ISO C, the sign of the remainder follows the dividend. `-7 % 3`
shall be `-1`.

```
begin test "negative modulo sign follows dividend"
  script
    echo $(( -7 % 3 ))
  expect
    stdout "-1"
    stderr ""
    exit_code 0
end test "negative modulo sign follows dividend"
```

#### Test: unary plus and unary minus

Unary `+` and `-` apply a sign to their operand. Unary plus is
effectively a no-op on a positive value; unary minus negates.

```
begin test "unary plus and unary minus"
  script
    echo $(( +5 )) $(( -5 )) $(( - -3 ))
  expect
    stdout "5 -5 3"
    stderr ""
    exit_code 0
end test "unary plus and unary minus"
```

#### Test: bitwise NOT

The `~` operator produces the bitwise complement (ones' complement) of
its operand. `~0` shall be `-1` on a two's-complement system.

```
begin test "bitwise NOT"
  script
    echo $(( ~0 ))
  expect
    stdout "-1"
    stderr ""
    exit_code 0
end test "bitwise NOT"
```

#### Test: logical NOT

The `!` operator yields `1` if its operand is zero, and `0` otherwise.

```
begin test "logical NOT"
  script
    echo $(( !0 )) $(( !1 )) $(( !42 ))
  expect
    stdout "1 0 0"
    stderr ""
    exit_code 0
end test "logical NOT"
```

#### Test: left shift

The `<<` operator shifts bits to the left. `1 << 4` shall be `16`.

```
begin test "left shift"
  script
    echo $(( 1 << 4 ))
  expect
    stdout "16"
    stderr ""
    exit_code 0
end test "left shift"
```

#### Test: right shift

The `>>` operator shifts bits to the right. `16 >> 3` shall be `2`.

```
begin test "right shift"
  script
    echo $(( 16 >> 3 ))
  expect
    stdout "2"
    stderr ""
    exit_code 0
end test "right shift"
```

#### Test: relational less-than and greater-than

The relational operators `<`, `<=`, `>`, `>=` yield `1` if the relation
is true, `0` otherwise.

```
begin test "relational less-than and greater-than"
  script
    echo $(( 3 < 5 )) $(( 5 < 3 )) $(( 3 <= 3 )) $(( 5 > 3 )) $(( 3 >= 4 )) $(( 4 >= 4 ))
  expect
    stdout "1 0 1 1 0 1"
    stderr ""
    exit_code 0
end test "relational less-than and greater-than"
```

#### Test: equality and inequality

The `==` operator yields `1` when operands are equal; `!=` yields `1`
when they differ.

```
begin test "equality and inequality"
  script
    echo $(( 5 == 5 )) $(( 5 == 6 )) $(( 5 != 6 )) $(( 5 != 5 ))
  expect
    stdout "1 0 1 0"
    stderr ""
    exit_code 0
end test "equality and inequality"
```

#### Test: bitwise AND

The `&` operator performs a bitwise AND. `0xFF & 0x0F` shall be `15`.

```
begin test "bitwise AND"
  script
    echo $(( 0xFF & 0x0F ))
  expect
    stdout "15"
    stderr ""
    exit_code 0
end test "bitwise AND"
```

#### Test: bitwise exclusive OR

The `^` operator performs a bitwise XOR. `0xFF ^ 0x0F` shall be `240`.

```
begin test "bitwise exclusive OR"
  script
    echo $(( 0xFF ^ 0x0F ))
  expect
    stdout "240"
    stderr ""
    exit_code 0
end test "bitwise exclusive OR"
```

#### Test: bitwise inclusive OR

The `|` operator performs a bitwise OR. `0xF0 | 0x0F` shall be `255`.

```
begin test "bitwise inclusive OR"
  script
    echo $(( 0xF0 | 0x0F ))
  expect
    stdout "255"
    stderr ""
    exit_code 0
end test "bitwise inclusive OR"
```

#### Test: logical AND short-circuit

The `&&` operator yields `1` if both operands are non-zero. It
short-circuits: if the left operand is zero, the right is not evaluated.

```
begin test "logical AND short-circuit"
  script
    x=0
    echo $(( 0 && (x = 5) ))
    echo "$x"
  expect
    stdout "0\n0"
    stderr ""
    exit_code 0
end test "logical AND short-circuit"
```

#### Test: logical OR short-circuit

The `||` operator yields `1` if either operand is non-zero. It
short-circuits: if the left operand is non-zero, the right is not evaluated.

```
begin test "logical OR short-circuit"
  script
    x=0
    echo $(( 1 || (x = 5) ))
    echo "$x"
  expect
    stdout "1\n0"
    stderr ""
    exit_code 0
end test "logical OR short-circuit"
```

#### Test: conditional (ternary) operator

The `expr ? expr : expr` conditional operator evaluates the second
expression if the condition is non-zero, or the third if zero.

```
begin test "conditional (ternary) operator"
  script
    echo $(( 1 ? 10 : 20 )) $(( 0 ? 10 : 20 ))
  expect
    stdout "10 20"
    stderr ""
    exit_code 0
end test "conditional (ternary) operator"
```

#### Test: ternary does not evaluate unused branch

The branch not selected by the ternary operator shall not be evaluated.
Side effects in the unused branch shall not occur.

```
begin test "ternary does not evaluate unused branch"
  script
    x=0
    echo $(( 1 ? 10 : (x = 99) ))
    echo "$x"
  expect
    stdout "10\n0"
    stderr ""
    exit_code 0
end test "ternary does not evaluate unused branch"
```

#### Test: simple assignment operator

The `=` assignment operator sets a variable and yields the assigned value.

```
begin test "simple assignment operator"
  script
    echo $(( x = 42 ))
    echo "$x"
  expect
    stdout "42\n42"
    stderr ""
    exit_code 0
end test "simple assignment operator"
```

#### Test: compound assignment operators

The compound assignment operators (`*=`, `/=`, `%=`, `+=`, `-=`,
`<<=`, `>>=`, `&=`, `^=`, `|=`) combine an operation with assignment.

```
begin test "compound assignment operators"
  script
    x=10; echo $(( x += 5 ))
    x=10; echo $(( x -= 3 ))
    x=10; echo $(( x *= 4 ))
    x=20; echo $(( x /= 3 ))
    x=17; echo $(( x %= 5 ))
    x=1;  echo $(( x <<= 4 ))
    x=32; echo $(( x >>= 2 ))
    x=15; echo $(( x &= 6 ))
    x=15; echo $(( x ^= 6 ))
    x=5;  echo $(( x |= 10 ))
  expect
    stdout "15\n7\n40\n6\n2\n16\n8\n6\n9\n15"
    stderr ""
    exit_code 0
end test "compound assignment operators"
```

#### Test: operator precedence multiplication before addition

Multiplication has higher precedence than addition, so `2 + 3 * 4`
evaluates as `2 + 12 = 14`, not `20`.

```
begin test "operator precedence multiplication before addition"
  script
    echo $(( 2 + 3 * 4 ))
  expect
    stdout "14"
    stderr ""
    exit_code 0
end test "operator precedence multiplication before addition"
```

#### Test: operator precedence bitwise shift vs addition

Additive operators have higher precedence than bitwise shift. `1 << 2 + 3`
evaluates as `1 << 5 = 32`, not `(1 << 2) + 3 = 7`.

```
begin test "operator precedence bitwise shift vs addition"
  script
    echo $(( 1 << 2 + 3 ))
  expect
    stdout "32"
    stderr ""
    exit_code 0
end test "operator precedence bitwise shift vs addition"
```

#### Test: operator precedence comparison vs bitwise AND

Equality operators have higher precedence than bitwise AND. `5 & 3 == 3`
evaluates as `5 & (3 == 3) = 5 & 1 = 1`, not `(5 & 3) == 3`.

```
begin test "operator precedence comparison vs bitwise AND"
  script
    echo $(( 5 & 3 == 3 ))
  expect
    stdout "1"
    stderr ""
    exit_code 0
end test "operator precedence comparison vs bitwise AND"
```

#### Test: operator precedence logical AND vs logical OR

Logical AND has higher precedence than logical OR. `0 || 1 && 0`
evaluates as `0 || (1 && 0) = 0 || 0 = 0`.

```
begin test "operator precedence logical AND vs logical OR"
  script
    echo $(( 0 || 1 && 0 ))
  expect
    stdout "0"
    stderr ""
    exit_code 0
end test "operator precedence logical AND vs logical OR"
```

#### Test: variables initialized to zero

All variables shall be initialized to zero if they are not otherwise
assigned. Referencing an unset variable in arithmetic shall yield zero.

```
begin test "variables initialized to zero"
  script
    unset undefined_arith_var
    echo $(( undefined_arith_var ))
    echo $(( undefined_arith_var + 5 ))
  expect
    stdout "0\n5"
    stderr ""
    exit_code 0
end test "variables initialized to zero"
```

#### Test: nested parentheses

Multiple levels of nested parentheses shall be evaluated correctly,
innermost first.

```
begin test "nested parentheses"
  script
    echo $(( ((2 + 3) * (4 - 1)) + 1 ))
  expect
    stdout "16"
    stderr ""
    exit_code 0
end test "nested parentheses"
```

#### Test: chained assignments

Assignment is right-associative. `a = b = c = 5` shall assign `5` to
all three variables.

```
begin test "chained assignments"
  script
    echo $(( a = b = c = 5 ))
    echo "$a $b $c"
  expect
    stdout "5\n5 5 5"
    stderr ""
    exit_code 0
end test "chained assignments"
```

#### Test: complex expression combining multiple operators

A complex expression exercises multiple operator categories and
precedence levels in a single evaluation.

```
begin test "complex expression combining multiple operators"
  script
    x=3
    echo $(( (x + 2) * 4 - 1 > 15 ? x << 2 : 0 ))
  expect
    stdout "12"
    stderr ""
    exit_code 0
end test "complex expression combining multiple operators"
```

#### Test: bitwise operators combined

Combining bitwise AND, OR, XOR, and NOT in a single expression to
verify they interact correctly with standard precedence.

```
begin test "bitwise operators combined"
  script
    echo $(( (~0xF0 & 0xFF) | 0x10 ^ 0x30 ))
  expect
    stdout "47"
    stderr ""
    exit_code 0
end test "bitwise operators combined"
```

#### Test: addition and subtraction

The binary `+` and `-` operators perform integer addition and
subtraction respectively, including with negative operands.

```
begin test "addition and subtraction"
  script
    echo $(( 3 + 4 )) $(( 10 - 7 )) $(( -3 + -4 )) $(( -10 - -7 ))
  expect
    stdout "7 3 -7 -3"
    stderr ""
    exit_code 0
end test "addition and subtraction"
```

#### Test: signed long minimum range

Integer variables and constants shall be implemented as equivalent to
the ISO C signed long data type, which is at least 32 bits. Values
at the boundaries of a 32-bit signed range (2147483647 and -2147483647)
shall be representable and arithmetic on them shall work correctly.

```
begin test "signed long minimum range"
  script
    echo $(( 2147483647 ))
    echo $(( -2147483647 ))
    echo $(( 2147483646 + 1 ))
    echo $(( -2147483646 - 1 ))
  expect
    stdout "2147483647\n-2147483647\n2147483647\n-2147483647"
    stderr ""
    exit_code 0
end test "signed long minimum range"
```

#### Test: octal and hexadecimal constants

The shell arithmetic shall support octal constants (leading `0`) and
hexadecimal constants (leading `0x` or `0X`), consistent with ISO C
integer constant syntax.

```
begin test "octal and hexadecimal constants"
  script
    echo $(( 010 )) $(( 0x1A )) $(( 0X1a ))
  expect
    stdout "8 26 26"
    stderr ""
    exit_code 0
end test "octal and hexadecimal constants"
```

#### Test: operator precedence ternary vs assignment

The conditional (ternary) operator has lower precedence than logical OR
but higher precedence than assignment. `x = 1 ? 10 : 20` assigns the
result of the ternary to `x`.

```
begin test "operator precedence ternary vs assignment"
  script
    echo $(( x = 1 ? 10 : 20 ))
    echo "$x"
  expect
    stdout "10\n10"
    stderr ""
    exit_code 0
end test "operator precedence ternary vs assignment"
```

#### Test: operator precedence bitwise OR vs logical AND

Bitwise inclusive OR has higher precedence than logical AND.
`1 && 0 | 2` evaluates as `1 && (0 | 2) = 1 && 2 = 1`.

```
begin test "operator precedence bitwise OR vs logical AND"
  script
    echo $(( 1 && 0 | 2 ))
  expect
    stdout "1"
    stderr ""
    exit_code 0
end test "operator precedence bitwise OR vs logical AND"
```

#### Test: operator precedence bitwise XOR vs bitwise OR

Bitwise exclusive OR has higher precedence than bitwise inclusive OR.
`3 | 5 ^ 7` evaluates as `3 | (5 ^ 7) = 3 | 2 = 3`.

```
begin test "operator precedence bitwise XOR vs bitwise OR"
  script
    echo $(( 3 | 5 ^ 7 ))
  expect
    stdout "3"
    stderr ""
    exit_code 0
end test "operator precedence bitwise XOR vs bitwise OR"
```

#### Test: operator precedence bitwise AND vs bitwise XOR

Bitwise AND has higher precedence than bitwise exclusive OR.
`6 ^ 7 & 3` evaluates as `6 ^ (7 & 3) = 6 ^ 3 = 5`.

```
begin test "operator precedence bitwise AND vs bitwise XOR"
  script
    echo $(( 6 ^ 7 & 3 ))
  expect
    stdout "5"
    stderr ""
    exit_code 0
end test "operator precedence bitwise AND vs bitwise XOR"
```

#### Test: operator precedence relational vs equality

Relational operators have higher precedence than equality operators.
`1 == 2 < 3` evaluates as `1 == (2 < 3) = 1 == 1 = 1`.

```
begin test "operator precedence relational vs equality"
  script
    echo $(( 1 == 2 < 3 ))
  expect
    stdout "1"
    stderr ""
    exit_code 0
end test "operator precedence relational vs equality"
```

#### Test: compound assignment yields assigned value

Each compound assignment operator shall yield the value that was
assigned, allowing the result to be used in a surrounding expression.

```
begin test "compound assignment yields assigned value"
  script
    x=10
    echo $(( (x += 5) * 2 ))
    echo "$x"
  expect
    stdout "30\n15"
    stderr ""
    exit_code 0
end test "compound assignment yields assigned value"
```

#### Test: logical operators yield strictly zero or one

Logical AND, logical OR, and logical NOT shall yield exactly `0` or `1`,
regardless of the magnitude of non-zero operands.

```
begin test "logical operators yield strictly zero or one"
  script
    echo $(( 42 && 99 )) $(( 42 || 99 )) $(( !0 )) $(( !999 ))
  expect
    stdout "1 1 1 0"
    stderr ""
    exit_code 0
end test "logical operators yield strictly zero or one"
```

#### Test: relational operators yield strictly zero or one

Relational and equality operators shall yield exactly `0` or `1`,
regardless of the magnitude of the operands.

```
begin test "relational operators yield strictly zero or one"
  script
    echo $(( 100 < 200 )) $(( 200 < 100 )) $(( 999 == 999 )) $(( 999 != 1000 ))
  expect
    stdout "1 0 1 1"
    stderr ""
    exit_code 0
end test "relational operators yield strictly zero or one"
```

#### Test: operator precedence unary vs additive

Unary operators have higher precedence than additive operators.
`~ 1 + 1` evaluates as `(~1) + 1 = -2 + 1 = -1`, not `~(1 + 1) = ~2 = -3`.

```
begin test "operator precedence unary vs additive"
  script
    echo $(( ~ 1 + 1 ))
  expect
    stdout "-1"
    stderr ""
    exit_code 0
end test "operator precedence unary vs additive"
```

#### Test: operator precedence shift vs relational

Bitwise shift operators have higher precedence than relational operators.
`1 < 1 << 2` evaluates as `1 < (1 << 2) = 1 < 4 = 1`, not
`(1 < 1) << 2 = 0 << 2 = 0`.

```
begin test "operator precedence shift vs relational"
  script
    echo $(( 1 < 1 << 2 ))
  expect
    stdout "1"
    stderr ""
    exit_code 0
end test "operator precedence shift vs relational"
```

#### Test: variable used before and after assignment in expression

A variable referenced on the right side of an assignment operator uses
the value the variable had before the assignment takes effect, consistent
with ISO C evaluation semantics.

```
begin test "variable used before and after assignment in expression"
  script
    x=5
    echo $(( x += x ))
    echo "$x"
  expect
    stdout "10\n10"
    stderr ""
    exit_code 0
end test "variable used before and after assignment in expression"
```

#### Test: nested ternary operators

The ternary operator is right-associative. `a ? b : c ? d : e` evaluates
as `a ? b : (c ? d : e)`.

```
begin test "nested ternary operators"
  script
    echo $(( 0 ? 1 : 0 ? 2 : 3 ))
    echo $(( 1 ? 10 : 0 ? 20 : 30 ))
    echo $(( 0 ? 10 : 1 ? 20 : 30 ))
  expect
    stdout "3\n10\n20"
    stderr ""
    exit_code 0
end test "nested ternary operators"
```

#### Test: bitwise NOT of non-zero value

The `~` operator produces the bitwise complement for arbitrary values,
not just zero. `~1` shall be `-2` and `~(-1)` shall be `0` on a
two's-complement system.

```
begin test "bitwise NOT of non-zero value"
  script
    echo $(( ~1 )) $(( ~(-1) )) $(( ~255 ))
  expect
    stdout "-2 0 -256"
    stderr ""
    exit_code 0
end test "bitwise NOT of non-zero value"
```
