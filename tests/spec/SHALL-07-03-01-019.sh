# SHALL-07-03-01-019
# "The <space>, which is part of the space and blank classes, cannot belong to
#  punct or graph, but shall automatically belong to the print class."
# Verify space is in [[:print:]] but not in [[:punct:]] or [[:graph:]].

LC_ALL=POSIX
export LC_ALL

fail=0

case ' ' in
  [[:print:]]) ;;
  *) printf '%s\n' "FAIL: space not matched by [[:print:]]" >&2; fail=1 ;;
esac

case ' ' in
  [[:graph:]]) printf '%s\n' "FAIL: space matched by [[:graph:]]" >&2; fail=1 ;;
  *) ;;
esac

case ' ' in
  [[:punct:]]) printf '%s\n' "FAIL: space matched by [[:punct:]]" >&2; fail=1 ;;
  *) ;;
esac

exit "$fail"
