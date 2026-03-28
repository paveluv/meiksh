# SHALL-19-29-14-001
# "If the trap name or number is invalid, a non-zero exit status shall be returned;
#  otherwise, zero shall be returned. For both interactive and non-interactive shells,
#  invalid signal names or numbers shall not be considered an error and shall not
#  cause the shell to abort."

# Valid trap -> exit 0
"$MEIKSH" -c 'trap "" INT; exit $?'
if [ $? -ne 0 ]; then
  printf '%s\n' "FAIL: valid trap did not return 0" >&2
  exit 1
fi

# Invalid signal -> non-zero, but shell continues
result=$("$MEIKSH" -c '
  trap "" NOSUCHSIG
  printf "continued\n"
')
case "$result" in
  *continued*) ;;
  *)
    printf '%s\n' "FAIL: shell aborted after invalid signal name" >&2
    exit 1
    ;;
esac
exit 0
