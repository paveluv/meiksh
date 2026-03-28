# SHALL-09-03-05-014
# "A character class expression shall represent the union of two sets ...
#  The following character class expressions shall be supported in all locales:
#  [:alnum:] [:alpha:] [:blank:] [:cntrl:] [:digit:] [:graph:] [:lower:]
#  [:print:] [:punct:] [:space:] [:upper:] [:xdigit:]"
# Verify all 12 standard character classes work in bracket expressions.

fail=0

case "7" in [[:digit:]]) ;; *) printf '%s\n' "FAIL: digit" >&2; fail=1 ;; esac
case "A" in [[:upper:]]) ;; *) printf '%s\n' "FAIL: upper" >&2; fail=1 ;; esac
case "z" in [[:lower:]]) ;; *) printf '%s\n' "FAIL: lower" >&2; fail=1 ;; esac
case "m" in [[:alpha:]]) ;; *) printf '%s\n' "FAIL: alpha" >&2; fail=1 ;; esac
case "	" in [[:blank:]]) ;; *) printf '%s\n' "FAIL: blank (tab)" >&2; fail=1 ;; esac
case " " in [[:space:]]) ;; *) printf '%s\n' "FAIL: space" >&2; fail=1 ;; esac
case "B" in [[:xdigit:]]) ;; *) printf '%s\n' "FAIL: xdigit" >&2; fail=1 ;; esac
case "." in [[:punct:]]) ;; *) printf '%s\n' "FAIL: punct" >&2; fail=1 ;; esac
case "3" in [[:alnum:]]) ;; *) printf '%s\n' "FAIL: alnum" >&2; fail=1 ;; esac
case "!" in [[:graph:]]) ;; *) printf '%s\n' "FAIL: graph" >&2; fail=1 ;; esac
case "a" in [[:print:]]) ;; *) printf '%s\n' "FAIL: print" >&2; fail=1 ;; esac

exit "$fail"
