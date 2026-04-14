use super::{
    append_history, check_mail, command_is_fc, expand_prompt, read_line, vi, write_prompt,
};
use crate::bstr::{BStrExt, ByteWriter};
use crate::shell::{Shell, ShellError};
use crate::sys;

pub(super) fn run_loop(shell: &mut Shell) -> Result<i32, ShellError> {
    let mut accumulated = Vec::<u8>::new();
    let mut sigchld_installed = false;
    loop {
        if shell.options.notify && !sigchld_installed {
            let _ = sys::install_shell_signal_handler(sys::SIGCHLD);
            sigchld_installed = true;
        } else if !shell.options.notify && sigchld_installed {
            let _ = sys::default_signal_action(sys::SIGCHLD);
            sigchld_installed = false;
        }

        for (id, state) in shell.reap_jobs() {
            use crate::shell::ReapedJobState;
            let msg = match state {
                ReapedJobState::Done(status, cmd) => {
                    if status == 0 {
                        ByteWriter::new()
                            .byte(b'[')
                            .usize_val(id)
                            .bytes(b"] Done\t")
                            .bytes(&cmd)
                            .byte(b'\n')
                            .finish()
                    } else {
                        ByteWriter::new()
                            .byte(b'[')
                            .usize_val(id)
                            .bytes(b"] Done(")
                            .i32_val(status)
                            .bytes(b")\t")
                            .bytes(&cmd)
                            .byte(b'\n')
                            .finish()
                    }
                }
                ReapedJobState::Signaled(sig, cmd) => ByteWriter::new()
                    .byte(b'[')
                    .usize_val(id)
                    .bytes(b"] Terminated (")
                    .bytes(sys::signal_name(sig))
                    .bytes(b")\t")
                    .bytes(&cmd)
                    .byte(b'\n')
                    .finish(),
                ReapedJobState::Stopped(sig, cmd) => ByteWriter::new()
                    .byte(b'[')
                    .usize_val(id)
                    .bytes(b"] Stopped (")
                    .bytes(sys::signal_name(sig))
                    .bytes(b")\t")
                    .bytes(&cmd)
                    .byte(b'\n')
                    .finish(),
            };
            let _ = sys::write_all_fd(sys::STDERR_FILENO, &msg);
        }

        shell.run_pending_traps()?;
        if !shell.running {
            break;
        }

        check_mail(shell);

        let prompt_str = if accumulated.is_empty() {
            expand_prompt(shell, b"PS1", b"$ ")
        } else {
            expand_prompt(shell, b"PS2", b"> ")
        };
        write_prompt(&prompt_str).map_err(|e| shell.diagnostic_syserr(1, &e))?;

        let line = match if shell.options.vi_mode {
            vi::read_line(shell, &prompt_str)
        } else {
            read_line()
        }
        .map_err(|e| shell.diagnostic_syserr(1, &e))?
        {
            Some(line) => line,
            None => {
                if !accumulated.is_empty() {
                    let _ = sys::write_all_fd(
                        sys::STDERR_FILENO,
                        b"meiksh: unexpected EOF while looking for matching token\n",
                    );
                    accumulated.clear();
                }
                break;
            }
        };
        if accumulated.is_empty() && line.trim_ascii_ws().is_empty() {
            continue;
        }
        accumulated.extend_from_slice(&line);

        match crate::syntax::parse_with_aliases(&accumulated, &shell.aliases) {
            Ok(_) => {}
            Err(ref e) if shell.input_is_incomplete(e) => {
                continue;
            }
            Err(_) => {}
        }

        let source = std::mem::take(&mut accumulated);
        let trimmed_end = {
            let mut end = source.len();
            while end > 0
                && (source[end - 1] == b' '
                    || source[end - 1] == b'\t'
                    || source[end - 1] == b'\n'
                    || source[end - 1] == b'\r')
            {
                end -= 1;
            }
            &source[..end]
        };
        append_history(shell, trimmed_end)?;
        let trimmed = source.trim_ascii_ws();
        if !command_is_fc(trimmed) {
            shell.add_history(trimmed);
        }
        match shell.execute_string(&source) {
            Ok(status) => shell.last_status = status,
            Err(error) => {
                shell.last_status = error.exit_status();
                continue;
            }
        }
        if !shell.running {
            break;
        }
    }

    Ok(shell.last_status)
}
