#!/bin/sh

# Test: XBD 8 Environment Variables
# Target: tests/matrix/tests/xbd_environment_variables.sh
#
# Tests the shell's handling and utilization of standard POSIX environment
# variables as specified in Base Definitions Chapter 8.

. "$MATRIX_DIR/lib.sh"

# REQUIREMENT: SHALL-XBD-8-3011:
# PATH This variable shall represent the sequence of path prefixes that certain
# functions and utilities apply in searching for an executable file.
mkdir -p mybin
cat << 'INEOF' > mybin/mytool
#!/bin/sh
echo "mytool executed"
INEOF
chmod +x mybin/mytool

test_cmd='
    PATH="$PWD/mybin:$PATH"
    mytool
'
assert_stdout "mytool executed" "$TARGET_SHELL -c '$test_cmd'"
rm -rf mybin

# REQUIREMENT: SHALL-XBD-8-3012:
# PWD This variable shall represent an absolute pathname of the current working
# directory.
test_cmd='
    echo "$PWD"
'
assert_stdout "$PWD" "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-XBD-8-3010:
# HOME The system shall initialize this variable at the time of login to be a
# pathname of the user's home directory.
mkdir -p myhome
test_cmd='
    HOME="$PWD/myhome"
    echo ~
'
assert_stdout "$PWD/myhome" "$TARGET_SHELL -c '$test_cmd'"
rm -rf myhome

# REQUIREMENT: SHALL-XBD-8-3000:
# LANG This variable shall determine the locale category for native language,
# local customs, and coded character set in the absence of the LC_ALL and other
# LC_* ( LC_COLLATE , LC_CTYPE , LC_MESSAGES , LC_MONETARY , LC_NUMERIC ,
# LC_TIME ) environment variables.
# REQUIREMENT: SHALL-XBD-8-3002:
# LC_ALL This variable shall determine the values for all locale categories.
# REQUIREMENT: SHALL-XBD-8-3003:
# LC_COLLATE This variable shall determine the locale category for character
# collation.
# REQUIREMENT: SHALL-XBD-8-3004:
# LC_CTYPE This variable shall determine the locale category for character
# handling functions, such as tolower () , toupper () , and isalpha ().
# REQUIREMENT: SHALL-XBD-8-3005:
# LC_MESSAGES This variable shall determine the locale category for processing
# affirmative and negative responses and the language and cultural conventions
# in which messages should be written.
# REQUIREMENT: SHALL-XBD-8-3007:
# If the LC_ALL environment variable is defined and is not null, the value of
# LC_ALL shall be used.
# REQUIREMENT: SHALL-XBD-8-3008:
# If the LC_* environment variable ( LC_COLLATE , LC_CTYPE , LC_MESSAGES ,
# LC_MONETARY , LC_NUMERIC , LC_TIME ) is defined and is not null, the value of
# the environment variable shall be used to initialize the category that
# corresponds to the environment variable.
# REQUIREMENT: SHALL-XBD-8-3009:
# If the LANG environment variable is defined and is not null, the value of the
# LANG environment variable shall be used.
# REQUIREMENT: SHALL-XBD-8-3001:
# The LANGUAGE environment variable shall be examined to determine the messages
# object to be used for the gettext family of functions or the gettext and
# ngettext utilities if NLSPATH is not set or the evaluation of NLSPATH did not
# lead to a suitable messages object being found.
# REQUIREMENT: SHALL-XBD-8-3006:
# This variable shall contain a sequence of templates to be used by catopen ()
# when attempting to locate message catalogs, and by the gettext family of
# functions when locating messages objects.

# Testing locale directly in a reliable, cross-platform way without assuming
# specific
# installed locales is difficult. However, we can assert that the shell passes
# these
# variables unmodified to child processes, allowing the system's locale
# mechanism to work.
test_cmd='
    export LANG=C
    export LC_ALL=C
    export LC_COLLATE=C
    export LC_CTYPE=C
    export LC_MESSAGES=C
    export LANGUAGE=C
    export NLSPATH=/dev/null
    env | grep -E "^(LANG|LC_ALL|LC_COLLATE|LC_CTYPE|LC_MESSAGES|LANGUAGE|NLSPATH)=" | sort
'
assert_stdout "LANG=C
LANGUAGE=C
LC_ALL=C
LC_COLLATE=C
LC_CTYPE=C
LC_MESSAGES=C
NLSPATH=/dev/null" "$TARGET_SHELL -c '$test_cmd'"

report
