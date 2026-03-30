use std::env;
use std::io::{self, Read, Write};
use std::os::unix::io::{FromRawFd, RawFd};
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::thread;

type CInt = libc::c_int;

#[repr(C)]
struct Winsize {
    ws_row: u16,
    ws_col: u16,
    ws_xpixel: u16,
    ws_ypixel: u16,
}

unsafe extern "C" {
    fn forkpty(
        amaster: *mut CInt,
        name: *mut libc::c_char,
        termp: *mut libc::termios,
        winp: *mut Winsize,
    ) -> libc::pid_t;
}

fn wifexited(status: CInt) -> bool {
    libc::WIFEXITED(status)
}

fn wexitstatus(status: CInt) -> i32 {
    libc::WEXITSTATUS(status)
}

fn do_forkpty() -> io::Result<(libc::pid_t, RawFd)> {
    unsafe {
        let mut master: CInt = -1;
        let mut termp: libc::termios = std::mem::zeroed();
        let mut winp: Winsize = std::mem::zeroed();
        winp.ws_row = 24;
        winp.ws_col = 80;

        let pid = forkpty(
            &mut master,
            std::ptr::null_mut(),
            &mut termp,
            &mut winp,
        );

        if pid < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok((pid, master as RawFd))
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <command> [args...]", args[0]);
        std::process::exit(1);
    }

    let cmd_str = &args[1];
    let cmd_args = &args[2..];

    match do_forkpty() {
        Ok((0, _)) => {
            // Child process
            let err = Command::new(cmd_str)
                .args(cmd_args)
                .exec();
            eprintln!("exec failed: {}", err);
            std::process::exit(1);
        }
        Ok((pid, master_fd)) => {
            // Parent process
            let mut master = unsafe { std::fs::File::from_raw_fd(master_fd) };
            
            // Forward stdin to PTY
            let mut stdin = io::stdin();
            let master_fd_clone = master_fd;
            
            let _writer = thread::spawn(move || {
                let mut buf = [0u8; 1024];
                let mut master_writer = unsafe { std::fs::File::from_raw_fd(master_fd_clone) };
                loop {
                    match stdin.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            if master_writer.write_all(&buf[..n]).is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            });

            // Read from PTY to stdout
            let mut stdout = io::stdout();
            let mut buf = [0u8; 1024];
            loop {
                match master.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if stdout.write_all(&buf[..n]).is_err() {
                            break;
                        }
                        let _ = stdout.flush();
                    }
                    Err(_) => break,
                }
            }
            
            unsafe {
                let mut status: CInt = 0;
                libc::waitpid(pid, &mut status, 0);
                if wifexited(status) {
                    std::process::exit(wexitstatus(status));
                } else {
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            eprintln!("forkpty failed: {}", e);
            std::process::exit(1);
        }
    }
}
