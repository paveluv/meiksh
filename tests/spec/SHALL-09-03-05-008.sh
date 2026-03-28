# SHALL-09-03-05-008
# "In the POSIX locale, a range expression represents the set of collating
#  elements that fall between two elements in the collation sequence, inclusive."
# "The <hyphen-minus> character shall be treated as itself if it occurs first
#  (after an initial '^', if any) or last in the list"
# Verify range expressions and literal hyphen handling.

# [a-z] matches lowercase in POSIX locale
_result=$(LC_ALL=C sh -c 'case m in [a-z]) printf match;; *) printf nomatch;; esac')
if [ "$_result" != "match" ]; then
  printf '%s\n' "FAIL: [a-z] did not match 'm' in POSIX locale" >&2
  exit 1
fi

# [a-z] does not match uppercase in POSIX locale
_result=$(LC_ALL=C sh -c 'case M in [a-z]) printf match;; *) printf nomatch;; esac')
if [ "$_result" != "nomatch" ]; then
  printf '%s\n' "FAIL: [a-z] matched 'M' in POSIX locale" >&2
  exit 1
fi

# [0-9] matches digit
case "5" in
  [0-9]) ;;
  *) printf '%s\n' "FAIL: [0-9] did not match '5'" >&2; exit 1 ;;
esac

# Literal hyphen when first
case "-" in
  [-a]) ;;
  *) printf '%s\n' "FAIL: [-a] did not match '-'" >&2; exit 1 ;;
esac

# Literal hyphen when last
case "-" in
  [a-]) ;;
  *) printf '%s\n' "FAIL: [a-] did not match '-'" >&2; exit 1 ;;
esac

exit 0
