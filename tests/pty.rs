#![allow(non_camel_case_types)]

use std::env;
use std::io::{self, Read, Write};
use std::os::unix::io::{FromRawFd, RawFd};
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::thread;

pub type c_int = i32;
pub type pid_t = i32;

#[repr(C)]
pub struct termios {
    pub c_iflag: usize,
    pub c_oflag: usize,
    pub c_cflag: usize,
    pub c_lflag: usize,
    pub c_cc: [u8; 20],
    pub c_ispeed: usize,
    pub c_ospeed: usize,
}

#[repr(C)]
pub struct winsize {
    pub ws_row: u16,
    pub ws_col: u16,
    pub ws_xpixel: u16,
    pub ws_ypixel: u16,
}

unsafe extern "C" {
    pub fn forkpty(
        amaster: *mut c_int,
        name: *mut i8,
        termp: *mut termios,
        winp: *mut winsize,
    ) -> pid_t;
    pub fn waitpid(pid: pid_t, status: *mut c_int, options: c_int) -> pid_t;
}

fn WIFEXITED(status: c_int) -> bool {
    (status & 0x7f) == 0
}

fn WEXITSTATUS(status: c_int) -> i32 {
    (status >> 8) & 0xff
}

pub fn do_forkpty() -> io::Result<(pid_t, RawFd)> {
    unsafe {
        let mut master: c_int = -1;
        let mut termp: termios = std::mem::zeroed();
        let mut winp: winsize = std::mem::zeroed();
        winp.ws_row = 24;
        winp.ws_col = 80;

        let pid = forkpty(
            &mut master as *mut _,
            std::ptr::null_mut(),
            &mut termp as *mut _,
            &mut winp as *mut _,
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
            
            let writer = thread::spawn(move || {
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
                let mut status = 0;
                waitpid(pid, &mut status, 0);
                if WIFEXITED(status) {
                    std::process::exit(WEXITSTATUS(status));
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
