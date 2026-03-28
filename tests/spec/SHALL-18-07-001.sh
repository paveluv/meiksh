# Test: SHALL-18-07-001
# Obligation: "intrinsic utilities are not subject to a PATH search during
#   command search and execution. The utilities named in Intrinsic Utilities
#   shall be intrinsic utilities."
# Verifies: Intrinsic utilities work even with empty PATH.

old_path="$PATH"

# cd is intrinsic - should work without PATH
PATH='' cd / || { printf '%s\n' "FAIL: cd should work without PATH" >&2; exit 1; }

# command is intrinsic
PATH='' command true || { printf '%s\n' "FAIL: command true without PATH" >&2; exit 1; }

PATH="$old_path"
exit 0
