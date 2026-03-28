# Test: SHALL-19-10-02-004
# Obligation: "When the TOKEN is exactly the reserved word esac, the token
#   identifier for esac shall result. Otherwise, the token WORD shall be
#   returned."
# Verifies: esac is recognized as reserved word at case-termination position.

result=""
case "test" in
    test) result="matched" ;;
esac
if [ "$result" != "matched" ]; then
    printf '%s\n' "FAIL: esac not recognized as case terminator" >&2
    exit 1
fi

# esac as a pattern word (not reserved) — used as a pattern
result=""
case "esac" in
    esac) result="matched_esac" ;;
esac
if [ "$result" != "matched_esac" ]; then
    printf '%s\n' "FAIL: esac as pattern word not handled correctly" >&2
    exit 1
fi

exit 0
