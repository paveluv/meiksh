# SHALL-20-62-04-001
# "The jobs utility shall conform to XBD 12.2 Utility Syntax Guidelines."
# Verify jobs accepts standard option syntax and -- terminator.

# Test 1: -l option accepted
_rc=$(sh -c 'sleep 60 & jobs -l >/dev/null 2>&1; _r=$?; kill $! 2>/dev/null; wait $! 2>/dev/null; exit $_r')
if [ $? -ne 0 ]; then
  printf '%s\n' "FAIL(1): jobs -l should be accepted" >&2
  exit 1
fi

# Test 2: -p option accepted
_rc=$(sh -c 'sleep 60 & jobs -p >/dev/null 2>&1; _r=$?; kill $! 2>/dev/null; wait $! 2>/dev/null; exit $_r')
if [ $? -ne 0 ]; then
  printf '%s\n' "FAIL(2): jobs -p should be accepted" >&2
  exit 1
fi

# Test 3: -- terminates options
_rc=$(sh -c 'sleep 60 & jobs -- >/dev/null 2>&1; _r=$?; kill $! 2>/dev/null; wait $! 2>/dev/null; exit $_r')
if [ $? -ne 0 ]; then
  printf '%s\n' "FAIL(3): jobs -- should be accepted" >&2
  exit 1
fi

exit 0
