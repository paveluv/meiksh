# SHALL-09-03-05-007
# "A character class expression shall represent the union of two sets:
#  The set of single characters that belong to the character class ...
#  The following character class expressions shall be supported in all locales:
#  [:alnum:] [:alpha:] [:blank:] [:cntrl:] [:digit:] [:graph:] [:lower:]
#  [:print:] [:punct:] [:space:] [:upper:] [:xdigit:]"
# Verify standard character classes work in bracket expressions.

case "5" in
  [[:digit:]]) ;;
  *) printf '%s\n' "FAIL: [[:digit:]] did not match '5'" >&2; exit 1 ;;
esac

case "Z" in
  [[:upper:]]) ;;
  *) printf '%s\n' "FAIL: [[:upper:]] did not match 'Z'" >&2; exit 1 ;;
esac

case "a" in
  [[:lower:]]) ;;
  *) printf '%s\n' "FAIL: [[:lower:]] did not match 'a'" >&2; exit 1 ;;
esac

case "a" in
  [[:alpha:]]) ;;
  *) printf '%s\n' "FAIL: [[:alpha:]] did not match 'a'" >&2; exit 1 ;;
esac

case " " in
  [[:blank:]]) ;;
  *) printf '%s\n' "FAIL: [[:blank:]] did not match ' '" >&2; exit 1 ;;
esac

case " " in
  [[:space:]]) ;;
  *) printf '%s\n' "FAIL: [[:space:]] did not match ' '" >&2; exit 1 ;;
esac

case "5" in
  [[:xdigit:]]) ;;
  *) printf '%s\n' "FAIL: [[:xdigit:]] did not match '5'" >&2; exit 1 ;;
esac

case "f" in
  [[:xdigit:]]) ;;
  *) printf '%s\n' "FAIL: [[:xdigit:]] did not match 'f'" >&2; exit 1 ;;
esac

case "!" in
  [[:punct:]]) ;;
  *) printf '%s\n' "FAIL: [[:punct:]] did not match '!'" >&2; exit 1 ;;
esac

case "a" in
  [[:alnum:]]) ;;
  *) printf '%s\n' "FAIL: [[:alnum:]] did not match 'a'" >&2; exit 1 ;;
esac

case "5" in
  [[:alnum:]]) ;;
  *) printf '%s\n' "FAIL: [[:alnum:]] did not match '5'" >&2; exit 1 ;;
esac

case "a" in
  [[:graph:]]) ;;
  *) printf '%s\n' "FAIL: [[:graph:]] did not match 'a'" >&2; exit 1 ;;
esac

case "a" in
  [[:print:]]) ;;
  *) printf '%s\n' "FAIL: [[:print:]] did not match 'a'" >&2; exit 1 ;;
esac

exit 0
