#!/usr/bin/env python3
"""Generate a self-contained POSIX shell benchmark.

Usage:
    python3 scripts/gen_bench.py > scripts/bench.sh
    $SHELL scripts/bench.sh

The generated script is a single file that runs under dash, bash --posix,
and meiksh. It reports per-section timings with 1-second precision, sweeps
locale-sensitive sections across available locales (C, C.UTF-8, and
en_US.UTF-8 when present), and keeps fixture setup out of the timed
sections (files for globbing, read-loops, and command-substitution are
created in *_setup helpers that run *outside* section_time).

Iteration counts in ITERS are tuned so each row takes ~10s on a release
build of meiksh. To recalibrate:

    1. Run: target/release/meiksh scripts/bench.sh
    2. For each row far from 10s, scale its ITERS entry by
       (10 / observed_seconds), rounded to a clean number.
    3. Regenerate and repeat until all rows are within +/-2s of 10s.

This file is the source of truth for the benchmark; the generated
scripts/bench.sh should never be edited by hand.
"""

from dataclasses import dataclass
from typing import Optional


# =============================================================================
# Iteration counts -- tuned for ~10s per row on release meiksh.
# =============================================================================
ITERS = {
    # Category A: pure compute
    "arith_int":         1_800_000,
    "arith_wide":        1_500_000,
    "var_assign":        2_500_000,
    "builtin_dispatch":  3_300_000,
    "func_call":         1_600_000,
    "control_flow":      1_400_000,
    "deep_parse":          550_000,
    "trap_set_unset":    2_700_000,
    # Category B: locale-sensitive (times below are per-locale; glob_class
    # is inherently slower under locales with non-C collation -- that's
    # the glob-sort strcoll cost being measured, not a calibration bug)
    "param_trim_ascii":    650_000,
    "param_trim_utf8":     650_000,
    "param_class_bracket": 1_800_000,
    "param_length":      3_300_000,
    "field_split":       1_300_000,
    "case_utf8_patterns":3_300_000,
    "glob_class":            7_000,
    # Category C: system I/O
    "fs_create_delete":      7_000,
    "fs_read_loop":          2_200,
    "io_write_append":   3_300_000,
    "io_heredoc":           15_000,
    "io_fd_redir":       2_000_000,
    "subshell_parens":      19_000,
    "cmd_sub_short":        17_000,
    "cmd_sub_long":          9_000,
    "pipeline":              6_000,
    # Category D: combined
    "combined_realistic":   27_000,
}

# Fixture sizes for *_setup helpers.
GLOB_FILES = 500             # *.txt + *.dat + *.log = 1500 files total
FS_READ_LOOP_LINES = 500     # lines in prebuilt file for fs_read_loop
CMD_SUB_LONG_LINES = 200     # lines in prebuilt file for cmd_sub_long


# =============================================================================
# Section registry.
# =============================================================================
@dataclass
class Section:
    name: str
    category: str  # "pure" | "locale" | "io" | "combined"
    body: str
    setup: Optional[str] = None


SECTIONS: list = []


def _add(name: str, category: str, body: str, *, setup: Optional[str] = None) -> None:
    SECTIONS.append(Section(name, category, body.rstrip(), setup=setup.rstrip() if setup else None))


# =============================================================================
# Category A -- pure compute (locale-neutral, each runs once).
# =============================================================================
_add(
    "arith_int", "pure",
    f"""\
  i=0
  while [ $i -lt {ITERS["arith_int"]} ]; do
    a=$(( i + 137 ))
    b=$(( a * 7 ))
    c=$(( b - 42 ))
    d=$(( c / 5 ))
    e=$(( d % 97 ))
    f=$(( a + b + c + d + e ))
    g=$(( f / 11 + 1 ))
    h=$(( g * g ))
    k=$(( h - i ))
    m=$(( k + 2 * i ))
    : "$a" "$b" "$c" "$d" "$e" "$f" "$g" "$h" "$k" "$m"
    i=$(( i + 1 ))
  done
""",
)

_add(
    "arith_wide", "pure",
    f"""\
  i=0
  while [ $i -lt {ITERS["arith_wide"]} ]; do
    a=$(( i << 20 ))
    b=$(( a >> 3 ))
    c=$(( a & 0xFFFFF ))
    d=$(( a | 0x1000 ))
    e=$(( c ^ d ))
    f=$(( ~e ))
    g=$(( a > 1000000 ? a - 1000000 : a + 1000000 ))
    h=$(( a != 0 && b != 0 ? 1 : 0 ))
    k=$(( g * 3 + e ))
    m=$(( -a + h ))
    : "$a" "$b" "$c" "$d" "$e" "$f" "$g" "$h" "$k" "$m"
    i=$(( i + 1 ))
  done
""",
)

_add(
    "var_assign", "pure",
    f"""\
  i=0
  while [ $i -lt {ITERS["var_assign"]} ]; do
    v1=foo
    v2=bar
    v3=baz
    v4="a little longer value with spaces"
    export V_EXPORTED=$v1
    unset V_EXPORTED
    unset v3
    v3=reborn
    : "$v1" "$v2" "$v3" "$v4"
    i=$(( i + 1 ))
  done
""",
)

_add(
    "builtin_dispatch", "pure",
    f"""\
  i=0
  while [ $i -lt {ITERS["builtin_dispatch"]} ]; do
    :
    true
    false || :
    [ x = x ]
    test -n x
    [ $i -ge 0 ]
    i=$(( i + 1 ))
  done
""",
)

_add(
    "func_call", "pure",
    f"""\
  i=0
  while [ $i -lt {ITERS["func_call"]} ]; do
    _fn_noop
    _fn_noop
    _fn_many_args $i 1 2 3 4
    _fn_many_args 5 6 7 8 $i
    : "$_FN_RESULT"
    i=$(( i + 1 ))
  done
""",
)

_add(
    "control_flow", "pure",
    f"""\
  i=0
  while [ $i -lt {ITERS["control_flow"]} ]; do
    r=0
    if [ $i -gt 0 ]; then
      if [ $i -lt 1000000000 ]; then
        if [ $(( i % 2 )) -eq 0 ]; then
          r=1
        else
          r=2
        fi
      else
        r=3
      fi
    else
      r=4
    fi
    case $(( i % 5 )) in
      0) t=alpha ;;
      1) t=bravo ;;
      2) t=charlie ;;
      3) t=delta ;;
      *) t=echo ;;
    esac
    [ $r -gt 0 ] && r=$(( r + 1 )) || r=0
    [ $r -ge 0 ] && r=$(( r + 1 )) || r=0
    [ $r -ne 99 ] && r=$(( r * 2 )) || :
    : "$r" "$t"
    i=$(( i + 1 ))
  done
""",
)

# deep_parse: re-parses a non-trivial script fragment on every iteration via
# eval. This isolates parse-time work (tokenising/AST-building the string) from
# the pre-parsed hot loop -- the outer `while` and `eval` overhead are shared
# across all shells, so what differs is how fast each shell turns the string
# into executable commands.
_add(
    "deep_parse", "pure",
    f"""\
  _src='
  x=1
  if [ $x -eq 1 ]; then
    if [ $x -gt 0 ]; then
      if [ $x -lt 100 ]; then
        if [ $x -ne 99 ]; then
          x=$(( x + 1 ))
        fi
      fi
    fi
  fi
  case $x in
    1) r=one ;;
    2) r=two ;;
    3) r=three ;;
    4) r=four ;;
    5) r=five ;;
    *) r=other ;;
  esac
  [ $x -ge 0 ] && : || :
  [ -n "$r" ] && : || :
  '
  i=0
  while [ $i -lt {ITERS["deep_parse"]} ]; do
    eval "$_src"
    i=$(( i + 1 ))
  done
""",
)

_add(
    "trap_set_unset", "pure",
    f"""\
  i=0
  while [ $i -lt {ITERS["trap_set_unset"]} ]; do
    trap ':' USR1
    trap ':' USR2
    trap - USR1
    trap - USR2
    i=$(( i + 1 ))
  done
""",
)


# =============================================================================
# Category B -- locale-sensitive (runs once per available locale).
# =============================================================================
_add(
    "param_trim_ascii", "locale",
    f"""\
  short='foo.bar.baz.qux.txt'
  longp='the-quick-brown-fox-jumps-over-the-lazy-dog/path/to/some/nested/file.tar.gz'
  i=0
  while [ $i -lt {ITERS["param_trim_ascii"]} ]; do
    a=${{short%.*}}
    b=${{short%%.*}}
    c=${{short#*.}}
    d=${{short##*.}}
    e=${{longp#*/}}
    f=${{longp##*/}}
    g=${{longp%/*}}
    h=${{longp%%/*}}
    j=${{longp%.tar.gz}}
    k=${{longp##*-}}
    : "$a" "$b" "$c" "$d" "$e" "$f" "$g" "$h" "$j" "$k"
    i=$(( i + 1 ))
  done
""",
)

_add(
    "param_trim_utf8", "locale",
    f"""\
  short='café.ünîcôdé.δοκιμή.テスト.tar.gz'
  longp='café-δοκιμή-テスト-тест/chemin/vers/dossier/ファイル.tar.gz'
  i=0
  while [ $i -lt {ITERS["param_trim_utf8"]} ]; do
    a=${{short%.*}}
    b=${{short%%.*}}
    c=${{short#*.}}
    d=${{short##*.}}
    e=${{longp#*/}}
    f=${{longp##*/}}
    g=${{longp%/*}}
    h=${{longp%%/*}}
    j=${{longp%.tar.gz}}
    k=${{longp##*-}}
    : "$a" "$b" "$c" "$d" "$e" "$f" "$g" "$h" "$j" "$k"
    i=$(( i + 1 ))
  done
""",
)

_add(
    "param_class_bracket", "locale",
    f"""\
  v1='abc123xyz'
  v2='hello-world-42'
  v3='δοκιμή123'
  i=0
  while [ $i -lt {ITERS["param_class_bracket"]} ]; do
    a=${{v1#[[:alpha:]]}}
    b=${{v1##[[:alpha:]]*}}
    c=${{v1%[0-9]}}
    d=${{v1%%[0-9]*}}
    e=${{v2#*-}}
    f=${{v2%[a-z]*}}
    g=${{v3#[[:alpha:]]*}}
    h=${{v3%[[:digit:]]*}}
    : "$a" "$b" "$c" "$d" "$e" "$f" "$g" "$h"
    i=$(( i + 1 ))
  done
""",
)

_add(
    "param_length", "locale",
    f"""\
  ascii='the-quick-brown-fox-jumps-over-the-lazy-dog'
  utf8='café-δοκιμή-テスト-тест-ünîcôdé'
  empty=''
  i=0
  while [ $i -lt {ITERS["param_length"]} ]; do
    a=${{#ascii}}
    b=${{#utf8}}
    c=${{#empty}}
    d=${{#ascii}}
    e=${{#utf8}}
    : "$a" "$b" "$c" "$d" "$e"
    i=$(( i + 1 ))
  done
""",
)

_add(
    "field_split", "locale",
    f"""\
  colon='alpha:bravo:charlie:delta:echo:foxtrot:golf:hotel'
  ws='aa bb cc dd ee ff gg hh ii jj kk ll mm nn oo pp'
  mixed='one,two;three four;five,six;seven'
  utf8list='café bravo δοκιμή テスト тест'
  i=0
  while [ $i -lt {ITERS["field_split"]} ]; do
    _old=$IFS
    IFS=:
    set -- $colon
    c1=$#
    IFS=' '
    set -- $ws
    c2=$#
    IFS=' ,;'
    set -- $mixed
    c3=$#
    IFS=' '
    set -- $utf8list
    c4=$#
    IFS=$_old
    : "$c1" "$c2" "$c3" "$c4"
    i=$(( i + 1 ))
  done
""",
)

_add(
    "case_utf8_patterns", "locale",
    f"""\
  i=0
  while [ $i -lt {ITERS["case_utf8_patterns"]} ]; do
    case $(( i % 5 )) in
      0) v=café ;;
      1) v=δοκιμή ;;
      2) v=テスト ;;
      3) v=тест ;;
      *) v=abc123 ;;
    esac
    case $v in
      'café') r=1 ;;
      'δοκιμή') r=2 ;;
      'テスト') r=3 ;;
      'тест') r=4 ;;
      [[:digit:]]*|*[[:digit:]]) r=5 ;;
      [[:alpha:]]*) r=6 ;;
      *) r=0 ;;
    esac
    case $v in
      *?*?*?*) s=long ;;
      *?*) s=multi ;;
      *) s=short ;;
    esac
    : "$r" "$s"
    i=$(( i + 1 ))
  done
""",
)

# glob_class: pre-create GLOB_FILES * 3 files once (idempotent setup); the
# timed body only invokes pathname expansion. Setup fires once per locale
# iteration but the [ -d ] guard short-circuits on subsequent calls.
_add(
    "glob_class", "locale",
    f"""\
  i=0
  while [ $i -lt {ITERS["glob_class"]} ]; do
    set -- "$BENCH_DIR/glob"/*.txt         ; c1=$#
    set -- "$BENCH_DIR/glob"/*.dat         ; c2=$#
    set -- "$BENCH_DIR/glob"/*_[0-9].txt   ; c3=$#
    set -- "$BENCH_DIR/glob"/file_1??.txt  ; c4=$#
    set -- "$BENCH_DIR/glob"/[[:alpha:]]*.log ; c5=$#
    : "$c1" "$c2" "$c3" "$c4" "$c5"
    i=$(( i + 1 ))
  done
""",
    setup=f"""\
  if [ -d "$BENCH_DIR/glob" ]; then return; fi
  mkdir -p "$BENCH_DIR/glob"
  _j=0
  while [ $_j -lt {GLOB_FILES} ]; do
    : > "$BENCH_DIR/glob/file_$_j.txt"
    : > "$BENCH_DIR/glob/data_$_j.dat"
    : > "$BENCH_DIR/glob/log_$_j.log"
    _j=$(( _j + 1 ))
  done
""",
)


# =============================================================================
# Category C -- system I/O (locale-neutral, each runs once).
# =============================================================================

# fs_create_delete: the filesystem-setup cost that used to be baked into
# bench_globbing, now measured on its own. Each iteration creates a dir,
# makes 20 files in it, then removes the whole dir.
_add(
    "fs_create_delete", "io",
    f"""\
  i=0
  while [ $i -lt {ITERS["fs_create_delete"]} ]; do
    mkdir "$BENCH_DIR/fs_$i"
    _j=0
    while [ $_j -lt 20 ]; do
      : > "$BENCH_DIR/fs_$i/f_$_j"
      _j=$(( _j + 1 ))
    done
    rm -rf "$BENCH_DIR/fs_$i"
    i=$(( i + 1 ))
  done
""",
)

_add(
    "fs_read_loop", "io",
    f"""\
  _f="$BENCH_DIR/read_loop.txt"
  i=0
  while [ $i -lt {ITERS["fs_read_loop"]} ]; do
    _count=0
    while IFS= read -r _line; do
      _count=$(( _count + 1 ))
    done < "$_f"
    : "$_count"
    i=$(( i + 1 ))
  done
""",
    setup=f"""\
  _f="$BENCH_DIR/read_loop.txt"
  if [ -s "$_f" ]; then return; fi
  _j=0
  while [ $_j -lt {FS_READ_LOOP_LINES} ]; do
    printf 'line %d alpha bravo charlie delta echo foxtrot\\n' $_j >> "$_f"
    _j=$(( _j + 1 ))
  done
""",
)

_add(
    "io_write_append", "io",
    f"""\
  _f="$BENCH_DIR/io_append.txt"
  : > "$_f"
  i=0
  while [ $i -lt {ITERS["io_write_append"]} ]; do
    echo "line $i foo bar baz quux alpha bravo charlie delta" >> "$_f"
    i=$(( i + 1 ))
  done
""",
)

_add(
    "io_heredoc", "io",
    f"""\
  _f="$BENCH_DIR/io_heredoc.txt"
  : > "$_f"
  _count=42
  i=0
  while [ $i -lt {ITERS["io_heredoc"]} ]; do
    cat >> "$_f" <<HEREDOC_END
Line $i: the quick brown fox jumps over the lazy dog
Expansion: arith=$(( i * 2 + 1 )) count=$_count
More text with "quotes" and special chars.
HEREDOC_END
    i=$(( i + 1 ))
  done
""",
)

_add(
    "io_fd_redir", "io",
    f"""\
  _f="$BENCH_DIR/io_fd.txt"
  : > "$_f"
  i=0
  while [ $i -lt {ITERS["io_fd_redir"]} ]; do
    echo "fd line $i" 3>"$_f" >&3
    i=$(( i + 1 ))
  done
""",
)

_add(
    "subshell_parens", "io",
    f"""\
  i=0
  while [ $i -lt {ITERS["subshell_parens"]} ]; do
    ( _x=$i; : "$_x" )
    ( _y=$(( i + 1 )); : "$_y" )
    i=$(( i + 1 ))
  done
""",
)

_add(
    "cmd_sub_short", "io",
    f"""\
  i=0
  while [ $i -lt {ITERS["cmd_sub_short"]} ]; do
    a=$(echo small)
    b=$(printf '%d' $i)
    : "$a" "$b"
    i=$(( i + 1 ))
  done
""",
)

_add(
    "cmd_sub_long", "io",
    f"""\
  _f="$BENCH_DIR/cmd_sub_long.txt"
  i=0
  while [ $i -lt {ITERS["cmd_sub_long"]} ]; do
    x=$(cat "$_f")
    : "$x"
    i=$(( i + 1 ))
  done
""",
    setup=f"""\
  _f="$BENCH_DIR/cmd_sub_long.txt"
  if [ -s "$_f" ]; then return; fi
  _j=0
  while [ $_j -lt {CMD_SUB_LONG_LINES} ]; do
    printf 'long line %d with some content padding text for size\\n' $_j >> "$_f"
    _j=$(( _j + 1 ))
  done
""",
)

_add(
    "pipeline", "io",
    f"""\
  i=0
  while [ $i -lt {ITERS["pipeline"]} ]; do
    echo "data $i" | cat > /dev/null
    echo "data $i" | cat | cat | cat > /dev/null
    i=$(( i + 1 ))
  done
""",
)


# =============================================================================
# Category D -- combined.
# =============================================================================
_add(
    "combined_realistic", "combined",
    f"""\
  template='deployment-app-v1.2.3-20240315-release-candidate'
  i=0
  while [ $i -lt {ITERS["combined_realistic"]} ]; do
    _name=${{template#*-}}
    _name=${{_name%%-*}}
    _version=${{template##*-v}}
    _version=${{_version%%-*}}
    _count=$(( i * 31 + ${{#template}} ))
    case $(( i % 3 )) in
      0) _action=build ;;
      1) _action=test ;;
      *) _action=deploy ;;
    esac
    _id=$(echo "$_action-$i")
    echo "$_id $_name $_version $_count" > /dev/null
    : "$_action"
    i=$(( i + 1 ))
  done
""",
)


# =============================================================================
# Emission.
# =============================================================================
HEADER = """\
#!/bin/sh
# Auto-generated POSIX shell benchmark -- do not edit by hand.
# Generated by scripts/gen_bench.py.
#
# Usage:
#   $SHELL scripts/bench.sh                    # run every section
#   $SHELL scripts/bench.sh NAME               # run a single section
#   $SHELL scripts/bench.sh NAME LOCALE        # run NAME under a specific
#                                              #   locale (locale-sensitive
#                                              #   sections only)
#   $SHELL scripts/bench.sh --list             # list available section names
#   $SHELL scripts/bench.sh --help             # show this help
#
# The single-section form is intended for profiling (e.g.
# `perf record -- target/release/meiksh scripts/bench.sh arith_int`): it
# skips banners/totals and emits only the one timed row.
#
# Each section targets ~10s on release meiksh. Locale-sensitive sections
# (Category B) run once per available locale (C, C.UTF-8, en_US.UTF-8 if
# present); the rest run once. Per-section setup (mkdir, file population,
# long-string initialisation) runs via a *_setup helper that is NOT
# included in the timed section; fixtures are cleaned up in bulk by the
# EXIT trap when the benchmark finishes.

set -e

BENCH_DIR=$(mktemp -d)
trap 'rm -rf "$BENCH_DIR"' EXIT

# Helper: print a second-precision duration for running "$@".
section_time() {
    _label=$1; shift
    _start=$(date +%s)
    "$@"
    _end=$(date +%s)
    printf "    %-42s %ss\\n" "$_label" "$(( _end - _start ))"
}

# Helper: run a bench with its setup (if defined) before timing.
run_bench() {
    _rb_label=$1
    _rb_fn=$2
    if command -v "${_rb_fn}_setup" >/dev/null 2>&1; then
        "${_rb_fn}_setup"
    fi
    section_time "$_rb_label" "$_rb_fn"
}

print_header() {
    printf "\\n-- %s --\\n" "$1"
}

print_sub_header() {
    printf "  -- locale: %s --\\n" "$1"
}

# Probe the host for the locales we want to exercise. Aliases (C.UTF-8 vs
# C.utf8, en_US.UTF-8 vs en_US.utf8) are de-duplicated: the first spelling
# that loads wins. Falls back to C if nothing else works.
detect_locales() {
    BENCH_LOCALES=''
    _have_cutf8=0
    _have_enus=0
    for _cand in C C.UTF-8 C.utf8 en_US.UTF-8 en_US.utf8; do
        case $_cand in
            C.UTF-8|C.utf8)
                if [ $_have_cutf8 -eq 1 ]; then continue; fi ;;
            en_US.UTF-8|en_US.utf8)
                if [ $_have_enus  -eq 1 ]; then continue; fi ;;
        esac
        if ( LC_ALL=$_cand /bin/sh -c ':' ) 2>/dev/null; then
            BENCH_LOCALES="$BENCH_LOCALES $_cand"
            case $_cand in
                C.UTF-8|C.utf8)         _have_cutf8=1 ;;
                en_US.UTF-8|en_US.utf8) _have_enus=1  ;;
            esac
        fi
    done
    [ -n "$BENCH_LOCALES" ] || BENCH_LOCALES=C
}

detect_locales
"""

# Small helper functions used by Category A sections (func_call).
PREAMBLE_FUNCTIONS = """\
_fn_noop() {
    :
}

_fn_many_args() {
    _a=$1; _b=$2; _c=$3; _d=$4; _e=$5
    _FN_RESULT=$(( _a + _b + _c + _d + _e ))
}
"""


def emit_section(section: Section) -> str:
    parts = []
    if section.setup:
        parts.append(f"bench_{section.name}_setup() {{\n{section.setup}\n}}\n")
    parts.append(f"bench_{section.name}() {{\n{section.body}\n}}\n")
    return "\n".join(parts)


def emit_main() -> str:
    pure = [s.name for s in SECTIONS if s.category == "pure"]
    locale = [s.name for s in SECTIONS if s.category == "locale"]
    io = [s.name for s in SECTIONS if s.category == "io"]
    combined = [s.name for s in SECTIONS if s.category == "combined"]

    lines: list = []

    # ------------------------------------------------------------------
    # Section name registries -- used by the arg dispatcher.
    # ------------------------------------------------------------------
    lines += [
        f'PURE_SECTIONS="{" ".join(pure)}"',
        f'LOCALE_SECTIONS="{" ".join(locale)}"',
        f'IO_SECTIONS="{" ".join(io)}"',
        f'COMBINED_SECTIONS="{" ".join(combined)}"',
        '',
    ]

    # ------------------------------------------------------------------
    # Dispatch helpers.
    # ------------------------------------------------------------------
    lines += [
        'usage() {',
        '    cat <<EOF',
        'Usage:',
        '  $0                    run every section',
        '  $0 NAME               run a single section (profile-friendly: no banner / no TOTAL)',
        '  $0 NAME LOCALE        run a locale-sensitive section under a specific locale',
        '  $0 --list | -l        list available section names',
        '  $0 --help | -h        show this help',
        'EOF',
        '}',
        '',
        'list_sections() {',
        '    printf "Pure compute:\\n  %s\\n"       "$PURE_SECTIONS"',
        '    printf "Locale-sensitive:\\n  %s\\n"   "$LOCALE_SECTIONS"',
        '    printf "  (detected locales:%s)\\n"    "$BENCH_LOCALES"',
        '    printf "System I/O:\\n  %s\\n"         "$IO_SECTIONS"',
        '    printf "Combined:\\n  %s\\n"           "$COMBINED_SECTIONS"',
        '}',
        '',
        '# True iff $1 appears as a space-separated token in $2.',
        '_in_list() {',
        '    case " $2 " in *" $1 "*) return 0 ;; esac',
        '    return 1',
        '}',
        '',
        '# Set LC_ALL/LANG to a specific locale (no save -- caller controls that).',
        '_set_locale() {',
        '    LC_ALL=$1; export LC_ALL',
        '    LANG=$1;   export LANG',
        '}',
        '',
        '_save_locale() {',
        '    _saved_lc_all_was_set=${LC_ALL+1}; _saved_lc_all=${LC_ALL-}',
        '    _saved_lang_was_set=${LANG+1};     _saved_lang=${LANG-}',
        '}',
        '',
        '_restore_locale() {',
        '    if [ "$_saved_lc_all_was_set" = 1 ]; then',
        '        LC_ALL=$_saved_lc_all; export LC_ALL',
        '    else',
        '        unset LC_ALL',
        '    fi',
        '    if [ "$_saved_lang_was_set" = 1 ]; then',
        '        LANG=$_saved_lang; export LANG',
        '    else',
        '        unset LANG',
        '    fi',
        '}',
        '',
        '# Run a single named section. If the section is locale-sensitive and',
        '# $2 is non-empty, run it only under that locale; otherwise sweep all',
        '# detected locales. No banners, no TOTAL -- intended for profiling.',
        'run_one() {',
        '    _one_name=$1',
        '    _one_loc=${2:-}',
        '    _one_kind=',
        '    if _in_list "$_one_name" "$PURE_SECTIONS"; then',
        '        _one_kind=simple',
        '    elif _in_list "$_one_name" "$IO_SECTIONS"; then',
        '        _one_kind=simple',
        '    elif _in_list "$_one_name" "$COMBINED_SECTIONS"; then',
        '        _one_kind=simple',
        '    elif _in_list "$_one_name" "$LOCALE_SECTIONS"; then',
        '        _one_kind=locale',
        '    fi',
        '    case $_one_kind in',
        '        simple)',
        '            if [ -n "$_one_loc" ]; then',
        '                printf "bench.sh: section %s is not locale-sensitive; ignoring LOCALE\\n" "$_one_name" >&2',
        '            fi',
        '            run_bench "$_one_name" "bench_$_one_name"',
        '            ;;',
        '        locale)',
        '            _save_locale',
        '            if [ -n "$_one_loc" ]; then',
        '                _set_locale "$_one_loc"',
        '                run_bench "$_one_name" "bench_$_one_name"',
        '            else',
        '                for _loc in $BENCH_LOCALES; do',
        '                    _set_locale "$_loc"',
        '                    print_sub_header "$_loc"',
        '                    run_bench "$_one_name" "bench_$_one_name"',
        '                done',
        '            fi',
        '            _restore_locale',
        '            ;;',
        '        *)',
        '            printf "bench.sh: unknown section: %s\\n" "$_one_name" >&2',
        '            printf "Use --list to see available sections.\\n" >&2',
        '            exit 2',
        '            ;;',
        '    esac',
        '}',
        '',
    ]

    # ------------------------------------------------------------------
    # run_all: the full benchmark body, preserved from the pre-CLI version.
    # ------------------------------------------------------------------
    lines += [
        'run_all() {',
        '    printf "=== meiksh benchmark ===\\n"',
        '    printf "  locales:%s\\n" "$BENCH_LOCALES"',
        '    BENCH_START=$(date +%s)',
        '',
        '    print_header "Pure compute"',
    ]
    for name in pure:
        lines.append(f'    run_bench "{name}" "bench_{name}"')

    lines += [
        '',
        '    print_header "Locale-sensitive"',
        '    _save_locale',
        '    for _loc in $BENCH_LOCALES; do',
        '        _set_locale "$_loc"',
        '        print_sub_header "$_loc"',
    ]
    for name in locale:
        lines.append(f'        run_bench "{name}" "bench_{name}"')
    lines += [
        '    done',
        '    _restore_locale',
        '',
        '    print_header "System I/O"',
    ]
    for name in io:
        lines.append(f'    run_bench "{name}" "bench_{name}"')

    lines += [
        '',
        '    print_header "Combined"',
    ]
    for name in combined:
        lines.append(f'    run_bench "{name}" "bench_{name}"')

    lines += [
        '',
        '    BENCH_END=$(date +%s)',
        '    printf "\\n  TOTAL: %ss\\n" "$(( BENCH_END - BENCH_START ))"',
        '}',
        '',
    ]

    # ------------------------------------------------------------------
    # Arg dispatch.
    # ------------------------------------------------------------------
    lines += [
        'case ${1:-} in',
        '    --help|-h)  usage; exit 0 ;;',
        '    --list|-l)  list_sections; exit 0 ;;',
        '    "")         run_all ;;',
        '    -*)         usage >&2; exit 2 ;;',
        '    *)          run_one "$1" "${2:-}" ;;',
        'esac',
    ]

    return "\n".join(lines) + "\n"


def main() -> None:
    out = [HEADER, PREAMBLE_FUNCTIONS]
    for section in SECTIONS:
        out.append(emit_section(section))
    out.append(emit_main())
    print("\n".join(out))


if __name__ == "__main__":
    main()
