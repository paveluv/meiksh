# Test: Interactive Features
# Target: tests/matrix/tests/interactive.sh
#
# Welcome to the Terminal Simulation! This file spins up our custom Rust
# Pseudo-TTY (`tests/pty.rs`) to deceive the target shell into thinking
# it's talking to a real, live human user.
#
# Because POSIX demands specific behaviors from interactive shells—such as
# dynamically evaluating prompt variables (`$PS1`)—we must orchestrate a
# full terminal session to prove compliance.

. "$MATRIX_DIR/lib.sh"


# ==============================================================================
# The Face of the Shell: Prompt Strings ($PS1)
# ==============================================================================
# REQUIREMENT: SHALL-2-5-025: Parameters: PS1: Each time an interactive shell is
# ready to read a command, the value of this variable shall be subjected to
# parameter expansion...

# We'll construct a sequence of commands to pipe into our PTY. We set `PS1`
# to a known string, then echo a unique phrase, and finally exit.
# We sleep between strokes to simulate human typing and give the shell
# time to evaluate the new environment.
interactive_script=$(cat << 'EOF'
sleep 0.5
echo 'PS1="prompt> "'
sleep 0.5
echo 'echo interactive-test'
sleep 0.5
echo 'exit'
EOF
)

# We invoke the target shell strictly in interactive mode (`-i`) and pass
# it our simulated keystrokes.
cmd="( $interactive_script ) | \"$MATRIX_DIR/pty\" $TARGET_SHELL -i"

# We run the command and capture raw output from the PTY session.
actual=$(eval "$cmd" 2>&1)

# Did the shell dynamically update its prompt? We search the raw output
# block for our custom prompt and our echoed test phrase.
case "$actual" in
    *"prompt> interactive-test"*)
        pass
        ;;
    *)
        fail "Expected PS1 prompt change and 'interactive-test' output, got:" \
             "$actual"
        ;;
esac


report
