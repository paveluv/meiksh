#!/bin/sh
set -eu

meiksh_bin="${1:-target/debug/meiksh}"

printf 'startup:-c-:\n'
time "$meiksh_bin" -c :

printf '\nstartup:-n:\n'
printf 'echo ok\n' | time "$meiksh_bin" -n >/dev/null

printf '\npipeline:\n'
time "$meiksh_bin" -c 'printf hi | wc -c' >/dev/null
