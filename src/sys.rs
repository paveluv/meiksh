use std::ffi::{CStr, CString};
use std::io::{self, Read};
use std::sync::atomic::{AtomicUsize, Ordering};
use libc::{self, c_char, c_int, c_long, mode_t};

pub type Pid = libc::pid_t;
pub type FileModeMask = libc::mode_t;
type ClockTicks = libc::clock_t;

const SC_CLK_TCK: c_int = libc::_SC_CLK_TCK;
const F_GETFL: c_int = libc::F_GETFL;
const F_SETFL: c_int = libc::F_SETFL;
const O_NONBLOCK: c_int = libc::O_NONBLOCK;

pub const STDIN_FILENO: c_int = libc::STDIN_FILENO;
pub const STDOUT_FILENO: c_int = libc::STDOUT_FILENO;
pub const STDERR_FILENO: c_int = libc::STDERR_FILENO;
pub const SIGHUP: c_int = libc::SIGHUP;
pub const SIGINT: c_int = libc::SIGINT;
pub const SIGQUIT: c_int = libc::SIGQUIT;
pub const SIGABRT: c_int = libc::SIGABRT;
pub const SIGALRM: c_int = libc::SIGALRM;
pub const SIGCONT: c_int = libc::SIGCONT;
pub const SIGTERM: c_int = libc::SIGTERM;
pub const WNOHANG: c_int = libc::WNOHANG;
const EINTR: i32 = libc::EINTR;
const SIG_DFL_HANDLER: libc::sighandler_t = libc::SIG_DFL;
const SIG_IGN_HANDLER: libc::sighandler_t = libc::SIG_IGN;
const SIG_ERR_HANDLER: libc::sighandler_t = libc::SIG_ERR;

pub const O_RDONLY: c_int = libc::O_RDONLY;
pub const O_WRONLY: c_int = libc::O_WRONLY;
pub const O_RDWR: c_int = libc::O_RDWR;
pub const O_CREAT: c_int = libc::O_CREAT;
pub const O_TRUNC: c_int = libc::O_TRUNC;
pub const O_APPEND: c_int = libc::O_APPEND;
pub const O_EXCL: c_int = libc::O_EXCL;
pub const O_CLOEXEC: c_int = libc::O_CLOEXEC;

pub const F_OK: c_int = libc::F_OK;
pub const R_OK: c_int = libc::R_OK;
pub const W_OK: c_int = libc::W_OK;
pub const X_OK: c_int = libc::X_OK;

pub const S_IFMT: mode_t = libc::S_IFMT;
pub const S_IFDIR: mode_t = libc::S_IFDIR;
pub const S_IFREG: mode_t = libc::S_IFREG;
pub const S_IFIFO: mode_t = libc::S_IFIFO;
pub const S_IXUSR: mode_t = libc::S_IXUSR;
pub const S_IXGRP: mode_t = libc::S_IXGRP;
pub const S_IXOTH: mode_t = libc::S_IXOTH;

#[derive(Clone, Copy)]
pub(crate) struct Syscalls {
    getpid: fn() -> Pid,
    waitpid: fn(Pid, *mut c_int, c_int) -> Pid,
    kill: fn(Pid, c_int) -> c_int,
    signal: fn(c_int, libc::sighandler_t) -> libc::sighandler_t,
    isatty: fn(c_int) -> c_int,
    tcgetpgrp: fn(c_int) -> Pid,
    tcsetpgrp: fn(c_int, Pid) -> c_int,
    setpgid: fn(Pid, Pid) -> c_int,
    pipe: fn(*mut c_int) -> c_int,
    dup: fn(c_int) -> c_int,
    dup2: fn(c_int, c_int) -> c_int,
    close: fn(c_int) -> c_int,
    fcntl: fn(c_int, c_int, c_int) -> c_int,
    read: fn(c_int, *mut u8, usize) -> isize,
    umask: fn(FileModeMask) -> FileModeMask,
    times: fn(*mut libc::tms) -> ClockTicks,
    sysconf: fn(c_int) -> c_long,
    execvp: fn(*const c_char, *const *const c_char) -> c_int,
    // Filesystem syscalls
    open: fn(*const c_char, c_int, mode_t) -> c_int,
    write: fn(c_int, *const u8, usize) -> isize,
    stat: fn(*const c_char, *mut libc::stat) -> c_int,
    lstat: fn(*const c_char, *mut libc::stat) -> c_int,
    fstat: fn(c_int, *mut libc::stat) -> c_int,
    access: fn(*const c_char, c_int) -> c_int,
    chdir: fn(*const c_char) -> c_int,
    getcwd: fn(*mut c_char, usize) -> *mut c_char,
    opendir: fn(*const c_char) -> *mut libc::DIR,
    readdir: fn(*mut libc::DIR) -> *mut libc::dirent,
    closedir: fn(*mut libc::DIR) -> c_int,
    realpath: fn(*const c_char, *mut c_char) -> *mut c_char,
    readlink: fn(*const c_char, *mut c_char, usize) -> isize,
    unlink: fn(*const c_char) -> c_int,
    // Process syscalls
    fork: fn() -> Pid,
}

pub(crate) fn default_syscalls() -> Syscalls {
    Syscalls {
        getpid: || unsafe { libc::getpid() },
        waitpid: |pid, status, options| unsafe { libc::waitpid(pid, status, options) },
        kill: |pid, sig| unsafe { libc::kill(pid, sig) },
        signal: |sig, handler| unsafe { libc::signal(sig, handler) },
        isatty: |fd| unsafe { libc::isatty(fd) },
        tcgetpgrp: |fd| unsafe { libc::tcgetpgrp(fd) },
        tcsetpgrp: |fd, pgrp| unsafe { libc::tcsetpgrp(fd, pgrp) },
        setpgid: |pid, pgid| unsafe { libc::setpgid(pid, pgid) },
        pipe: |fds| unsafe { libc::pipe(fds) },
        dup: |fd| unsafe { libc::dup(fd) },
        dup2: |oldfd, newfd| unsafe { libc::dup2(oldfd, newfd) },
        close: |fd| unsafe { libc::close(fd) },
        fcntl: |fd, cmd, arg| unsafe { libc::fcntl(fd, cmd, arg) },
        read: |fd, buf, count| unsafe { libc::read(fd, buf.cast(), count) },
        umask: |cmask| unsafe { libc::umask(cmask) },
        times: |buffer| unsafe { libc::times(buffer) },
        sysconf: |name| unsafe { libc::sysconf(name) },
        execvp: |file, argv| unsafe { libc::execvp(file, argv) },
        open: |path, flags, mode| unsafe { libc::open(path, flags, mode as c_int) },
        write: |fd, buf, count| unsafe { libc::write(fd, buf.cast(), count) },
        stat: |path, buf| unsafe { libc::stat(path, buf) },
        lstat: |path, buf| unsafe { libc::lstat(path, buf) },
        fstat: |fd, buf| unsafe { libc::fstat(fd, buf) },
        access: |path, mode| unsafe { libc::access(path, mode) },
        chdir: |path| unsafe { libc::chdir(path) },
        getcwd: |buf, size| unsafe { libc::getcwd(buf, size) },
        opendir: |path| unsafe { libc::opendir(path) },
        readdir: |dirp| unsafe { libc::readdir(dirp) },
        closedir: |dirp| unsafe { libc::closedir(dirp) },
        realpath: |path, resolved| unsafe { libc::realpath(path, resolved) },
        readlink: |path, buf, bufsiz| unsafe { libc::readlink(path, buf, bufsiz) },
        unlink: |path| unsafe { libc::unlink(path) },
        fork: || unsafe { libc::fork() },
    }
}

static PENDING_SIGNALS: AtomicUsize = AtomicUsize::new(0);

extern "C" fn record_signal(sig: c_int) {
    if let Some(mask) = signal_mask(sig) {
        PENDING_SIGNALS.fetch_or(mask, Ordering::SeqCst);
    }
}

fn syscalls() -> Syscalls {
    #[cfg(test)]
    {
        return test_support::current_syscalls().unwrap_or_else(default_syscalls);
    }

    #[cfg(not(test))]
    {
        default_syscalls()
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use std::cell::RefCell;
    use std::sync::Mutex;

    thread_local! {
        static TEST_SYSCALLS: RefCell<Option<Syscalls>> = const { RefCell::new(None) };
        static TEST_PROCESS_IDS: RefCell<Option<(libc::uid_t, libc::uid_t, libc::gid_t, libc::gid_t)>> =
            const { RefCell::new(None) };
    }

    fn syscall_lock() -> &'static Mutex<()> {
        static LOCK: Mutex<()> = Mutex::new(());
        &LOCK
    }

    fn pending_signal_lock() -> &'static Mutex<()> {
        static LOCK: Mutex<()> = Mutex::new(());
        &LOCK
    }

    pub(crate) fn current_syscalls() -> Option<Syscalls> {
        TEST_SYSCALLS.with(|cell| *cell.borrow())
    }

    pub(crate) fn current_process_ids() -> Option<(libc::uid_t, libc::uid_t, libc::gid_t, libc::gid_t)> {
        TEST_PROCESS_IDS.with(|cell| *cell.borrow())
    }

    pub(crate) fn with_test_syscalls<T>(syscalls: Syscalls, f: impl FnOnce() -> T) -> T {
        let _guard = syscall_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        TEST_SYSCALLS.with(|cell| {
            let previous = cell.replace(Some(syscalls));
            let result = f();
            cell.replace(previous);
            result
        })
    }

    pub(crate) fn with_execvp_for_test<T>(
        execvp_fn: fn(*const c_char, *const *const c_char) -> c_int,
        f: impl FnOnce() -> T,
    ) -> T {
        let syscalls = Syscalls {
            execvp: execvp_fn,
            ..default_syscalls()
        };
        with_test_syscalls(syscalls, f)
    }

    pub(crate) fn with_signal_syscall_for_test<T>(
        signal_fn: fn(c_int, libc::sighandler_t) -> libc::sighandler_t,
        f: impl FnOnce() -> T,
    ) -> T {
        let syscalls = Syscalls {
            signal: signal_fn,
            ..default_syscalls()
        };
        with_test_syscalls(syscalls, f)
    }

    pub(crate) fn with_waitpid_for_test<T>(
        waitpid_fn: fn(Pid, *mut c_int, c_int) -> Pid,
        f: impl FnOnce() -> T,
    ) -> T {
        let syscalls = Syscalls {
            waitpid: waitpid_fn,
            ..default_syscalls()
        };
        with_test_syscalls(syscalls, f)
    }

    pub(crate) fn set_pending_signals_for_test(signals: &[c_int]) {
        let bits = signals
            .iter()
            .filter_map(|signal| signal_mask(*signal))
            .fold(0usize, |acc, bit| acc | bit);
        PENDING_SIGNALS.store(bits, Ordering::SeqCst);
    }

    pub(crate) fn with_pending_signals_for_test<T>(signals: &[c_int], f: impl FnOnce() -> T) -> T {
        let _guard = pending_signal_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let previous = PENDING_SIGNALS.load(Ordering::SeqCst);
        set_pending_signals_for_test(signals);
        let result = f();
        PENDING_SIGNALS.store(previous, Ordering::SeqCst);
        result
    }

    pub(crate) fn with_job_control_syscalls_for_test<T>(
        isatty_fn: fn(c_int) -> c_int,
        tcgetpgrp_fn: fn(c_int) -> Pid,
        tcsetpgrp_fn: fn(c_int, Pid) -> c_int,
        setpgid_fn: fn(Pid, Pid) -> c_int,
        kill_fn: fn(Pid, c_int) -> c_int,
        f: impl FnOnce() -> T,
    ) -> T {
        let syscalls = Syscalls {
            isatty: isatty_fn,
            tcgetpgrp: tcgetpgrp_fn,
            tcsetpgrp: tcsetpgrp_fn,
            setpgid: setpgid_fn,
            kill: kill_fn,
            ..default_syscalls()
        };
        with_test_syscalls(syscalls, f)
    }

    pub(crate) fn with_fd_ops_for_test<T>(
        dup_fn: fn(c_int) -> c_int,
        dup2_fn: fn(c_int, c_int) -> c_int,
        close_fn: fn(c_int) -> c_int,
        f: impl FnOnce() -> T,
    ) -> T {
        let syscalls = Syscalls {
            dup: dup_fn,
            dup2: dup2_fn,
            close: close_fn,
            ..default_syscalls()
        };
        with_test_syscalls(syscalls, f)
    }

    pub(crate) fn with_fcntl_and_isatty_for_test<T>(
        fcntl_fn: fn(c_int, c_int, c_int) -> c_int,
        isatty_fn: fn(c_int) -> c_int,
        f: impl FnOnce() -> T,
    ) -> T {
        let syscalls = Syscalls {
            fcntl: fcntl_fn,
            isatty: isatty_fn,
            ..default_syscalls()
        };
        with_test_syscalls(syscalls, f)
    }

    pub(crate) fn with_waitpid_and_kill_for_test<T>(
        waitpid_fn: fn(Pid, *mut c_int, c_int) -> Pid,
        kill_fn: fn(Pid, c_int) -> c_int,
        f: impl FnOnce() -> T,
    ) -> T {
        let syscalls = Syscalls {
            waitpid: waitpid_fn,
            kill: kill_fn,
            ..default_syscalls()
        };
        with_test_syscalls(syscalls, f)
    }

    pub(crate) fn with_umask_for_test<T>(
        umask_fn: fn(FileModeMask) -> FileModeMask,
        f: impl FnOnce() -> T,
    ) -> T {
        let syscalls = Syscalls { umask: umask_fn, ..default_syscalls() };
        with_test_syscalls(syscalls, f)
    }

    pub(crate) fn with_times_error_for_test<T>(f: impl FnOnce() -> T) -> T {
        fn fake_times(_buffer: *mut libc::tms) -> ClockTicks {
            ClockTicks::MAX
        }
        fn fake_sysconf(_name: c_int) -> c_long {
            60
        }
        let syscalls = Syscalls {
            times: fake_times,
            sysconf: fake_sysconf,
            ..default_syscalls()
        };
        with_test_syscalls(syscalls, f)
    }

    pub(crate) fn with_process_ids_for_test<T>(
        ids: (libc::uid_t, libc::uid_t, libc::gid_t, libc::gid_t),
        f: impl FnOnce() -> T,
    ) -> T {
        TEST_PROCESS_IDS.with(|cell| {
            let previous = cell.replace(Some(ids));
            let result = f();
            cell.replace(previous);
            result
        })
    }

    // --- In-Memory VFS for tests ---

    use std::collections::HashMap;
    use std::path::PathBuf;

    #[derive(Clone, Debug)]
    struct VfsFile {
        contents: Vec<u8>,
        mode: mode_t,
    }

    #[derive(Clone, Debug)]
    pub(crate) struct VfsState {
        files: HashMap<PathBuf, VfsFile>,
        dirs: std::collections::HashSet<PathBuf>,
        cwd: PathBuf,
        fd_table: HashMap<c_int, VfsFd>,
        next_fd: c_int,
    }

    #[derive(Clone, Debug)]
    struct VfsFd {
        path: PathBuf,
        offset: usize,
        #[allow(dead_code)]
        flags: c_int,
    }

    thread_local! {
        static VFS_STATE: RefCell<Option<VfsState>> = const { RefCell::new(None) };
    }

    fn with_vfs<R>(f: impl FnOnce(&mut VfsState) -> R) -> R {
        VFS_STATE.with(|cell| {
            let mut borrow = cell.borrow_mut();
            f(borrow.as_mut().expect("VFS not initialized"))
        })
    }

    pub(crate) struct VfsBuilder {
        state: VfsState,
    }

    impl VfsBuilder {
        pub(crate) fn new() -> Self {
            let mut dirs = std::collections::HashSet::new();
            dirs.insert(PathBuf::from("/"));
            Self {
                state: VfsState {
                    files: HashMap::new(),
                    dirs,
                    cwd: PathBuf::from("/"),
                    fd_table: HashMap::new(),
                    next_fd: 100,
                },
            }
        }

        pub(crate) fn file(mut self, path: &str, contents: &[u8]) -> Self {
            let p = PathBuf::from(path);
            if let Some(parent) = p.parent() {
                self.ensure_dirs(parent);
            }
            self.state.files.insert(
                p,
                VfsFile {
                    contents: contents.to_vec(),
                    mode: 0o644,
                },
            );
            self
        }

        pub(crate) fn file_with_mode(mut self, path: &str, contents: &[u8], mode: mode_t) -> Self {
            let p = PathBuf::from(path);
            if let Some(parent) = p.parent() {
                self.ensure_dirs(parent);
            }
            self.state.files.insert(
                p,
                VfsFile {
                    contents: contents.to_vec(),
                    mode,
                },
            );
            self
        }

        pub(crate) fn dir(mut self, path: &str) -> Self {
            self.ensure_dirs(&PathBuf::from(path));
            self
        }

        pub(crate) fn cwd(mut self, path: &str) -> Self {
            self.ensure_dirs(&PathBuf::from(path));
            self.state.cwd = PathBuf::from(path);
            self
        }

        fn ensure_dirs(&mut self, path: &std::path::Path) {
            let mut current = PathBuf::new();
            for component in path.components() {
                current.push(component);
                self.state.dirs.insert(current.clone());
            }
        }

        fn vfs_syscalls() -> Syscalls {
            Syscalls {
                open: vfs_open,
                write: vfs_write,
                read: vfs_read,
                close: vfs_close,
                stat: vfs_stat,
                lstat: vfs_stat,
                fstat: vfs_fstat,
                access: vfs_access,
                chdir: vfs_chdir,
                getcwd: vfs_getcwd,
                opendir: vfs_opendir,
                readdir: vfs_readdir,
                closedir: vfs_closedir,
                realpath: vfs_realpath,
                readlink: vfs_readlink,
                unlink: vfs_unlink,
                ..default_syscalls()
            }
        }

        pub(crate) fn build(self) -> (Syscalls, VfsState) {
            (Self::vfs_syscalls(), self.state)
        }

        pub(crate) fn run<T>(self, f: impl FnOnce() -> T) -> T {
            let (syscalls, state) = self.build();
            VFS_STATE.with(|cell| {
                let previous = cell.replace(Some(state));
                let result = with_test_syscalls(syscalls, f);
                cell.replace(previous);
                result
            })
        }

        pub(crate) fn run_with_fd_ops<T>(
            self,
            dup_fn: fn(c_int) -> c_int,
            dup2_fn: fn(c_int, c_int) -> c_int,
            close_fn: fn(c_int) -> c_int,
            f: impl FnOnce() -> T,
        ) -> T {
            let (mut syscalls, state) = self.build();
            syscalls.dup = dup_fn;
            syscalls.dup2 = dup2_fn;
            syscalls.close = close_fn;
            VFS_STATE.with(|cell| {
                let previous = cell.replace(Some(state));
                let result = with_test_syscalls(syscalls, f);
                cell.replace(previous);
                result
            })
        }

        pub(crate) fn run_with_waitpid<T>(
            self,
            waitpid_fn: fn(Pid, *mut c_int, c_int) -> Pid,
            f: impl FnOnce() -> T,
        ) -> T {
            let (mut syscalls, state) = self.build();
            syscalls.waitpid = waitpid_fn;
            VFS_STATE.with(|cell| {
                let previous = cell.replace(Some(state));
                let result = with_test_syscalls(syscalls, f);
                cell.replace(previous);
                result
            })
        }

        pub(crate) fn run_with_fcntl_and_isatty<T>(
            self,
            fcntl_fn: fn(c_int, c_int, c_int) -> c_int,
            isatty_fn: fn(c_int) -> c_int,
            f: impl FnOnce() -> T,
        ) -> T {
            let (mut syscalls, state) = self.build();
            syscalls.fcntl = fcntl_fn;
            syscalls.isatty = isatty_fn;
            VFS_STATE.with(|cell| {
                let previous = cell.replace(Some(state));
                let result = with_test_syscalls(syscalls, f);
                cell.replace(previous);
                result
            })
        }
    }

    fn vfs_resolve(path_ptr: *const c_char) -> PathBuf {
        let cstr = unsafe { CStr::from_ptr(path_ptr) };
        let s = cstr.to_str().unwrap_or("");
        let p = PathBuf::from(s);
        if p.is_absolute() {
            p
        } else {
            with_vfs(|state| state.cwd.join(&p))
        }
    }

    pub(crate) fn set_errno_val(errno: c_int) {
        unsafe { *libc::__error() = errno; }
    }

    fn vfs_open(path: *const c_char, flags: c_int, mode: mode_t) -> c_int {
        let resolved = vfs_resolve(path);
        with_vfs(|state| {
            let creating = flags & super::O_CREAT != 0;
            let truncating = flags & super::O_TRUNC != 0;
            let exclusive = flags & super::O_EXCL != 0;
            let appending = flags & super::O_APPEND != 0;

            if state.dirs.contains(&resolved) {
                set_errno_val(libc::EISDIR);
                return -1;
            }

            if exclusive && creating && state.files.contains_key(&resolved) {
                set_errno_val(libc::EEXIST);
                return -1;
            }

            if creating {
                if !state.files.contains_key(&resolved) {
                    let parent = resolved.parent().unwrap_or(std::path::Path::new("/"));
                    if !state.dirs.contains(parent) {
                        set_errno_val(libc::ENOENT);
                        return -1;
                    }
                    state.files.insert(
                        resolved.clone(),
                        VfsFile { contents: Vec::new(), mode },
                    );
                }
            }

            if !state.files.contains_key(&resolved) {
                set_errno_val(libc::ENOENT);
                return -1;
            }

            if truncating {
                if let Some(file) = state.files.get_mut(&resolved) {
                    file.contents.clear();
                }
            }

            let offset = if appending {
                state.files.get(&resolved).map_or(0, |f| f.contents.len())
            } else {
                0
            };

            let fd = state.next_fd;
            state.next_fd += 1;
            state.fd_table.insert(
                fd,
                VfsFd {
                    path: resolved,
                    offset,
                    flags,
                },
            );
            fd
        })
    }

    fn vfs_write(fd: c_int, buf: *const u8, count: usize) -> isize {
        let in_vfs = with_vfs(|state| state.fd_table.contains_key(&fd));
        if !in_vfs {
            return unsafe { libc::write(fd, buf as *const libc::c_void, count) };
        }
        with_vfs(|state| {
            let vfd = state.fd_table.get_mut(&fd).unwrap();
            let path = vfd.path.clone();
            let offset = vfd.offset;
            let data = unsafe { std::slice::from_raw_parts(buf, count) };
            if let Some(file) = state.files.get_mut(&path) {
                if offset >= file.contents.len() {
                    file.contents.extend_from_slice(data);
                } else {
                    let end = offset + data.len();
                    if end > file.contents.len() {
                        file.contents.resize(end, 0);
                    }
                    file.contents[offset..end].copy_from_slice(data);
                }
                vfd.offset = offset + data.len();
                count as isize
            } else {
                set_errno_val(libc::EBADF);
                -1
            }
        })
    }

    fn vfs_read(fd: c_int, buf: *mut u8, count: usize) -> isize {
        let in_vfs = with_vfs(|state| state.fd_table.contains_key(&fd));
        if !in_vfs {
            return unsafe { libc::read(fd, buf as *mut libc::c_void, count) };
        }
        with_vfs(|state| {
            let vfd = state.fd_table.get_mut(&fd).unwrap();
            let path = vfd.path.clone();
            let offset = vfd.offset;
            if let Some(file) = state.files.get(&path) {
                let available = file.contents.len().saturating_sub(offset);
                let to_read = count.min(available);
                if to_read > 0 {
                    unsafe {
                        std::ptr::copy_nonoverlapping(
                            file.contents[offset..].as_ptr(),
                            buf,
                            to_read,
                        );
                    }
                }
                vfd.offset += to_read;
                to_read as isize
            } else {
                0
            }
        })
    }

    fn vfs_close(fd: c_int) -> c_int {
        let in_vfs = with_vfs(|state| state.fd_table.contains_key(&fd));
        if !in_vfs {
            return unsafe { libc::close(fd) };
        }
        with_vfs(|state| {
            state.fd_table.remove(&fd);
            0
        })
    }

    fn vfs_fill_stat(stat_buf: *mut libc::stat, mode: mode_t, size: u64) {
        unsafe {
            std::ptr::write_bytes(stat_buf, 0, 1);
            (*stat_buf).st_mode = mode;
            (*stat_buf).st_size = size as i64;
        }
    }

    fn vfs_stat(path: *const c_char, buf: *mut libc::stat) -> c_int {
        let resolved = vfs_resolve(path);
        with_vfs(|state| {
            if let Some(file) = state.files.get(&resolved) {
                vfs_fill_stat(buf, super::S_IFREG | file.mode, file.contents.len() as u64);
                0
            } else if state.dirs.contains(&resolved) {
                vfs_fill_stat(buf, super::S_IFDIR | 0o755, 0);
                0
            } else {
                set_errno_val(libc::ENOENT);
                -1
            }
        })
    }

    fn vfs_fstat(fd: c_int, buf: *mut libc::stat) -> c_int {
        with_vfs(|state| {
            let Some(vfd) = state.fd_table.get(&fd) else {
                set_errno_val(libc::EBADF);
                return -1;
            };
            let path = vfd.path.clone();
            if let Some(file) = state.files.get(&path) {
                vfs_fill_stat(buf, super::S_IFREG | file.mode, file.contents.len() as u64);
                0
            } else if state.dirs.contains(&path) {
                vfs_fill_stat(buf, super::S_IFDIR | 0o755, 0);
                0
            } else {
                set_errno_val(libc::EBADF);
                -1
            }
        })
    }

    fn vfs_access(path: *const c_char, mode: c_int) -> c_int {
        let resolved = vfs_resolve(path);
        with_vfs(|state| {
            if state.files.contains_key(&resolved) || state.dirs.contains(&resolved) {
                if mode == super::X_OK {
                    if let Some(file) = state.files.get(&resolved) {
                        if file.mode & 0o111 == 0 {
                            set_errno_val(libc::EACCES);
                            return -1;
                        }
                    }
                }
                if mode == super::R_OK {
                    if let Some(file) = state.files.get(&resolved) {
                        if file.mode & 0o444 == 0 {
                            set_errno_val(libc::EACCES);
                            return -1;
                        }
                    }
                }
                0
            } else {
                set_errno_val(libc::ENOENT);
                -1
            }
        })
    }

    fn vfs_chdir(path: *const c_char) -> c_int {
        let resolved = vfs_resolve(path);
        with_vfs(|state| {
            if state.dirs.contains(&resolved) {
                state.cwd = resolved;
                0
            } else {
                set_errno_val(libc::ENOENT);
                -1
            }
        })
    }

    fn vfs_getcwd(buf: *mut c_char, size: usize) -> *mut c_char {
        with_vfs(|state| {
            let cwd_str = state.cwd.display().to_string();
            let needed = cwd_str.len() + 1;
            if needed > size {
                set_errno_val(libc::ERANGE);
                return std::ptr::null_mut();
            }
            unsafe {
                std::ptr::copy_nonoverlapping(cwd_str.as_ptr().cast(), buf, cwd_str.len());
                *buf.add(cwd_str.len()) = 0;
            }
            buf
        })
    }

    static VFS_DIR_ENTRIES: Mutex<Vec<(usize, Vec<String>, usize)>> = Mutex::new(Vec::new());

    fn vfs_opendir(path: *const c_char) -> *mut libc::DIR {
        let resolved = vfs_resolve(path);
        let entries: Vec<String> = with_vfs(|state| {
            if !state.dirs.contains(&resolved) {
                return Vec::new();
            }
            let mut names = Vec::new();
            for file_path in state.files.keys() {
                if file_path.parent() == Some(&resolved) {
                    if let Some(name) = file_path.file_name() {
                        names.push(name.to_string_lossy().into_owned());
                    }
                }
            }
            for dir_path in &state.dirs {
                if dir_path.parent() == Some(&resolved) && dir_path != &resolved {
                    if let Some(name) = dir_path.file_name() {
                        names.push(name.to_string_lossy().into_owned());
                    }
                }
            }
            names.sort();
            names.dedup();
            names
        });
        if entries.is_empty() {
            let resolved_check = vfs_resolve(path);
            let is_dir = with_vfs(|state| state.dirs.contains(&resolved_check));
            if !is_dir {
                set_errno_val(libc::ENOENT);
                return std::ptr::null_mut();
            }
        }
        let mut guard = VFS_DIR_ENTRIES.lock().unwrap();
        let id = guard.len() + 1;
        guard.push((id, entries, 0));
        id as *mut libc::DIR
    }

    fn vfs_readdir(dirp: *mut libc::DIR) -> *mut libc::dirent {
        thread_local! {
            static DIRENT_BUF: RefCell<libc::dirent> = RefCell::new(unsafe { std::mem::zeroed() });
        }
        let id = dirp as usize;
        let mut guard = VFS_DIR_ENTRIES.lock().unwrap();
        let Some(entry) = guard.iter_mut().find(|(eid, _, _)| *eid == id) else {
            return std::ptr::null_mut();
        };
        let (_, entries, index) = entry;
        if *index >= entries.len() {
            return std::ptr::null_mut();
        }
        let name = &entries[*index];
        *index += 1;
        DIRENT_BUF.with(|cell| {
            let mut dirent = cell.borrow_mut();
            let bytes = name.as_bytes();
            let len = bytes.len().min(255);
            unsafe {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), dirent.d_name.as_mut_ptr().cast(), len);
                *dirent.d_name.as_mut_ptr().add(len) = 0;
            }
            &mut *dirent as *mut libc::dirent
        })
    }

    fn vfs_closedir(dirp: *mut libc::DIR) -> c_int {
        let id = dirp as usize;
        let mut guard = VFS_DIR_ENTRIES.lock().unwrap();
        guard.retain(|(eid, _, _)| *eid != id);
        0
    }

    fn vfs_realpath(path: *const c_char, resolved: *mut c_char) -> *mut c_char {
        let p = vfs_resolve(path);
        let exists = with_vfs(|state| {
            state.files.contains_key(&p) || state.dirs.contains(&p)
        });
        if !exists {
            set_errno_val(libc::ENOENT);
            return std::ptr::null_mut();
        }
        let s = p.display().to_string();
        if resolved.is_null() {
            let c = CString::new(s).unwrap();
            unsafe { libc::strdup(c.as_ptr()) }
        } else {
            let bytes = s.as_bytes();
            unsafe {
                std::ptr::copy_nonoverlapping(bytes.as_ptr().cast(), resolved, bytes.len());
                *resolved.add(bytes.len()) = 0;
            }
            resolved
        }
    }

    fn vfs_readlink(_path: *const c_char, _buf: *mut c_char, _bufsiz: usize) -> isize {
        set_errno_val(libc::EINVAL);
        -1
    }

    fn vfs_unlink(path: *const c_char) -> c_int {
        let resolved = vfs_resolve(path);
        with_vfs(|state| {
            if state.files.remove(&resolved).is_some() {
                0
            } else {
                set_errno_val(libc::ENOENT);
                -1
            }
        })
    }

    // --- FakeSpawn infrastructure for tests ---

    #[derive(Clone, Debug)]
    pub(crate) struct FakeChild {
        pub(crate) pid: Pid,
        pub(crate) exit_status: c_int,
        pub(crate) stdout_data: Vec<u8>,
    }

    #[derive(Clone, Debug)]
    #[allow(dead_code)]
    pub(crate) struct FakeSpawnState {
        children: Vec<FakeChild>,
        spawn_index: usize,
        pipe_fds: HashMap<c_int, Vec<u8>>,
        next_pipe_fd: c_int,
    }

    thread_local! {
        static FAKE_SPAWN_STATE: RefCell<Option<FakeSpawnState>> =
            const { RefCell::new(None) };
    }

    fn with_fake_spawn<R>(f: impl FnOnce(&mut FakeSpawnState) -> R) -> R {
        FAKE_SPAWN_STATE.with(|cell| {
            let mut borrow = cell.borrow_mut();
            f(borrow.as_mut().expect("FakeSpawn not initialized"))
        })
    }

    pub(crate) struct FakeSpawnBuilder {
        children: Vec<FakeChild>,
        next_pid: Pid,
    }

    impl FakeSpawnBuilder {
        pub(crate) fn new() -> Self {
            Self {
                children: Vec::new(),
                next_pid: 1000,
            }
        }

        pub(crate) fn child(mut self, exit_status: i32, stdout_data: &[u8]) -> Self {
            self.children.push(FakeChild {
                pid: self.next_pid,
                exit_status,
                stdout_data: stdout_data.to_vec(),
            });
            self.next_pid += 1;
            self
        }

        #[allow(dead_code)]
        pub(crate) fn child_with_pid(mut self, pid: Pid, exit_status: i32, stdout_data: &[u8]) -> Self {
            self.children.push(FakeChild {
                pid,
                exit_status,
                stdout_data: stdout_data.to_vec(),
            });
            self
        }

        fn fake_spawn_syscalls() -> Syscalls {
            Syscalls {
                fork: fake_fork,
                execvp: fake_execvp,
                waitpid: fake_waitpid,
                pipe: fake_pipe,
                read: fake_spawn_read,
                close: fake_spawn_close,
                dup2: fake_spawn_dup2,
                setpgid: |_, _| 0,
                ..default_syscalls()
            }
        }

        pub(crate) fn build(self) -> (Syscalls, FakeSpawnState) {
            let state = FakeSpawnState {
                children: self.children,
                spawn_index: 0,
                pipe_fds: HashMap::new(),
                next_pipe_fd: 200,
            };
            (Self::fake_spawn_syscalls(), state)
        }

        pub(crate) fn run<T>(self, f: impl FnOnce() -> T) -> T {
            let (syscalls, state) = self.build();
            FAKE_SPAWN_STATE.with(|cell| {
                let previous = cell.replace(Some(state));
                let result = with_test_syscalls(syscalls, f);
                cell.replace(previous);
                result
            })
        }
    }

    impl VfsBuilder {
        #[allow(dead_code)]
        pub(crate) fn with_fake_spawn(self, spawn: FakeSpawnBuilder) -> VfsWithFakeSpawn {
            VfsWithFakeSpawn { vfs: self, spawn }
        }
    }

    #[allow(dead_code)]
    pub(crate) struct VfsWithFakeSpawn {
        vfs: VfsBuilder,
        spawn: FakeSpawnBuilder,
    }

    impl VfsWithFakeSpawn {
        #[allow(dead_code)]
        pub(crate) fn run<T>(self, f: impl FnOnce() -> T) -> T {
            let (_, vfs_state) = self.vfs.build();
            let (spawn_syscalls, spawn_state) = self.spawn.build();
            let combined = Syscalls {
                open: vfs_open,
                write: vfs_write,
                stat: vfs_stat,
                lstat: vfs_stat,
                fstat: vfs_fstat,
                access: vfs_access,
                chdir: vfs_chdir,
                getcwd: vfs_getcwd,
                opendir: vfs_opendir,
                readdir: vfs_readdir,
                closedir: vfs_closedir,
                realpath: vfs_realpath,
                readlink: vfs_readlink,
                unlink: vfs_unlink,
                ..spawn_syscalls
            };

            VFS_STATE.with(|vcell| {
                let vprev = vcell.replace(Some(vfs_state));
                let result = FAKE_SPAWN_STATE.with(|scell| {
                    let sprev = scell.replace(Some(spawn_state));
                    let result = with_test_syscalls(combined, f);
                    scell.replace(sprev);
                    result
                });
                vcell.replace(vprev);
                result
            })
        }
    }

    fn fake_fork() -> Pid {
        with_fake_spawn(|state| {
            if state.spawn_index < state.children.len() {
                let child = &state.children[state.spawn_index];
                let pid = child.pid;
                state.spawn_index += 1;
                pid
            } else {
                set_errno_val(libc::EAGAIN);
                -1
            }
        })
    }

    fn fake_execvp(_file: *const c_char, _argv: *const *const c_char) -> c_int {
        unsafe { libc::_exit(0) };
    }

    fn fake_waitpid(pid: Pid, status: *mut c_int, _options: c_int) -> Pid {
        with_fake_spawn(|state| {
            if let Some(child) = state.children.iter().find(|c| c.pid == pid) {
                unsafe {
                    *status = child.exit_status << 8;
                }
                pid
            } else if pid == -1 {
                if let Some(child) = state.children.first() {
                    let pid = child.pid;
                    let status_val = child.exit_status;
                    unsafe {
                        *status = status_val << 8;
                    }
                    pid
                } else {
                    set_errno_val(libc::ECHILD);
                    -1
                }
            } else {
                set_errno_val(libc::ECHILD);
                -1
            }
        })
    }

    fn fake_pipe(fds: *mut c_int) -> c_int {
        with_fake_spawn(|state| {
            let r = state.next_pipe_fd;
            let w = state.next_pipe_fd + 1;
            state.next_pipe_fd += 2;

            if state.spawn_index < state.children.len() {
                let data = state.children[state.spawn_index].stdout_data.clone();
                state.pipe_fds.insert(r, data);
            } else {
                state.pipe_fds.insert(r, Vec::new());
            }
            state.pipe_fds.insert(w, Vec::new());

            unsafe {
                *fds = r;
                *fds.add(1) = w;
            }
            0
        })
    }

    fn fake_spawn_read(fd: c_int, buf: *mut u8, count: usize) -> isize {
        let result = FAKE_SPAWN_STATE.with(|cell| {
            let borrow = cell.borrow();
            if let Some(state) = borrow.as_ref() {
                if let Some(data) = state.pipe_fds.get(&fd) {
                    let to_read = count.min(data.len());
                    if to_read > 0 {
                        unsafe {
                            std::ptr::copy_nonoverlapping(data.as_ptr(), buf, to_read);
                        }
                    }
                    return Some(to_read as isize);
                }
            }
            None
        });

        if let Some(n) = result {
            if n > 0 {
                FAKE_SPAWN_STATE.with(|cell| {
                    let mut borrow = cell.borrow_mut();
                    if let Some(state) = borrow.as_mut() {
                        if let Some(data) = state.pipe_fds.get_mut(&fd) {
                            let consumed = n as usize;
                            *data = data[consumed..].to_vec();
                        }
                    }
                });
            }
            n
        } else {
            let vfs_result = VFS_STATE.with(|cell| {
                let borrow = cell.borrow();
                borrow.is_some()
            });
            if vfs_result {
                vfs_read(fd, buf, count)
            } else {
                set_errno_val(libc::EBADF);
                -1
            }
        }
    }

    fn fake_spawn_close(fd: c_int) -> c_int {
        let removed = FAKE_SPAWN_STATE.with(|cell| {
            let mut borrow = cell.borrow_mut();
            if let Some(state) = borrow.as_mut() {
                state.pipe_fds.remove(&fd).is_some()
            } else {
                false
            }
        });
        if removed {
            return 0;
        }
        let vfs_result = VFS_STATE.with(|cell| {
            let borrow = cell.borrow();
            borrow.is_some()
        });
        if vfs_result {
            vfs_close(fd)
        } else {
            0
        }
    }

    fn fake_spawn_dup2(oldfd: c_int, newfd: c_int) -> c_int {
        let _ = (oldfd, newfd);
        newfd
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WaitStatus {
    pub pid: Pid,
    pub status: c_int,
}

pub fn current_pid() -> Pid {
    (syscalls().getpid)()
}

pub fn is_interactive_fd(fd: c_int) -> bool {
    (syscalls().isatty)(fd) == 1
}

pub fn has_same_real_and_effective_ids() -> bool {
    #[cfg(test)]
    if let Some((uid, euid, gid, egid)) = test_support::current_process_ids() {
        return uid == euid && gid == egid;
    }
    unsafe { libc::getuid() == libc::geteuid() && libc::getgid() == libc::getegid() }
}

pub fn wait_pid(pid: Pid, nohang: bool) -> io::Result<Option<WaitStatus>> {
    let mut status = 0;
    let options = if nohang { WNOHANG } else { 0 };
    let result = (syscalls().waitpid)(pid, &mut status, options);
    if result > 0 {
        Ok(Some(WaitStatus { pid: result, status }))
    } else if result == 0 {
        Ok(None)
    } else {
        Err(io::Error::last_os_error())
    }
}

pub fn send_signal(pid: Pid, signal: c_int) -> io::Result<()> {
    let result = (syscalls().kill)(pid, signal);
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

pub fn install_shell_signal_handler(signal: c_int) -> io::Result<()> {
    let result = (syscalls().signal)(signal, record_signal as *const () as libc::sighandler_t);
    if result == SIG_ERR_HANDLER {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub fn ignore_signal(signal: c_int) -> io::Result<()> {
    let result = (syscalls().signal)(signal, SIG_IGN_HANDLER);
    if result == SIG_ERR_HANDLER {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub fn default_signal_action(signal: c_int) -> io::Result<()> {
    let result = (syscalls().signal)(signal, SIG_DFL_HANDLER);
    if result == SIG_ERR_HANDLER {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub fn has_pending_signal() -> Option<c_int> {
    let bits = PENDING_SIGNALS.load(Ordering::SeqCst);
    supported_trap_signals()
        .into_iter()
        .find(|signal| signal_mask(*signal).map(|mask| bits & mask != 0).unwrap_or(false))
}

pub fn take_pending_signals() -> Vec<c_int> {
    let bits = PENDING_SIGNALS.swap(0, Ordering::SeqCst);
    supported_trap_signals()
        .into_iter()
        .filter(|signal| signal_mask(*signal).map(|mask| bits & mask != 0).unwrap_or(false))
        .collect()
}

pub fn supported_trap_signals() -> Vec<c_int> {
    vec![SIGHUP, SIGINT, SIGQUIT, SIGABRT, SIGALRM, SIGTERM]
}

pub fn interrupted(error: &io::Error) -> bool {
    error.raw_os_error() == Some(EINTR)
}

pub fn current_foreground_pgrp(fd: c_int) -> io::Result<Pid> {
    let result = (syscalls().tcgetpgrp)(fd);
    if result >= 0 {
        Ok(result)
    } else {
        Err(io::Error::last_os_error())
    }
}

pub fn set_foreground_pgrp(fd: c_int, pgrp: Pid) -> io::Result<()> {
    let result = (syscalls().tcsetpgrp)(fd, pgrp);
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

pub fn set_process_group(pid: Pid, pgid: Pid) -> io::Result<()> {
    let result = (syscalls().setpgid)(pid, pgid);
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

pub fn create_pipe() -> io::Result<(c_int, c_int)> {
    let mut fds = [0; 2];
    let result = (syscalls().pipe)(fds.as_mut_ptr());
    if result == 0 {
        Ok((fds[0], fds[1]))
    } else {
        Err(io::Error::last_os_error())
    }
}

pub fn duplicate_fd(oldfd: c_int, newfd: c_int) -> io::Result<()> {
    let result = (syscalls().dup2)(oldfd, newfd);
    if result >= 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

pub fn duplicate_fd_to_new(fd: c_int) -> io::Result<c_int> {
    let result = (syscalls().dup)(fd);
    if result >= 0 {
        Ok(result)
    } else {
        Err(io::Error::last_os_error())
    }
}

pub fn close_fd(fd: c_int) -> io::Result<()> {
    let result = (syscalls().close)(fd);
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

fn fd_status_flags(fd: c_int) -> io::Result<c_int> {
    let result = (syscalls().fcntl)(fd, F_GETFL, 0);
    if result >= 0 {
        Ok(result)
    } else {
        Err(io::Error::last_os_error())
    }
}

fn set_fd_status_flags(fd: c_int, flags: c_int) -> io::Result<()> {
    let result = (syscalls().fcntl)(fd, F_SETFL, flags);
    if result >= 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

fn fifo_like_fd(fd: c_int) -> bool {
    let mut buf = std::mem::MaybeUninit::<libc::stat>::zeroed();
    let result = (syscalls().fstat)(fd, buf.as_mut_ptr());
    if result != 0 {
        return false;
    }
    let buf = unsafe { buf.assume_init() };
    (buf.st_mode & S_IFMT) == S_IFIFO
}

pub fn ensure_blocking_read_fd(fd: c_int) -> io::Result<()> {
    if !is_interactive_fd(fd) && !fifo_like_fd(fd) {
        return Ok(());
    }
    let flags = fd_status_flags(fd)?;
    if flags & O_NONBLOCK != 0 {
        set_fd_status_flags(fd, flags & !O_NONBLOCK)?;
    }
    Ok(())
}

pub fn read_fd(fd: c_int, buf: &mut [u8]) -> io::Result<usize> {
    let result = (syscalls().read)(fd, buf.as_mut_ptr(), buf.len());
    if result >= 0 {
        Ok(result as usize)
    } else {
        Err(io::Error::last_os_error())
    }
}

pub struct FdReader {
    fd: c_int,
}

impl FdReader {
    pub fn new(fd: c_int) -> Self {
        Self { fd }
    }
}

impl Read for FdReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        read_fd(self.fd, buf)
    }
}

// --- Filesystem wrapper functions ---

#[derive(Clone, Debug)]
pub struct FileStat {
    pub mode: mode_t,
    pub size: u64,
}

impl FileStat {
    pub fn is_dir(&self) -> bool {
        (self.mode & S_IFMT) == S_IFDIR
    }

    pub fn is_regular_file(&self) -> bool {
        (self.mode & S_IFMT) == S_IFREG
    }

    pub fn is_executable(&self) -> bool {
        self.mode & (S_IXUSR | S_IXGRP | S_IXOTH) != 0
    }
}

fn to_cstring(path: &str) -> io::Result<CString> {
    CString::new(path).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path contains null byte"))
}

fn stat_raw(path: &str) -> io::Result<libc::stat> {
    let c_path = to_cstring(path)?;
    let mut buf = std::mem::MaybeUninit::<libc::stat>::zeroed();
    let result = (syscalls().stat)(c_path.as_ptr(), buf.as_mut_ptr());
    if result == 0 {
        Ok(unsafe { buf.assume_init() })
    } else {
        Err(io::Error::last_os_error())
    }
}

fn lstat_raw(path: &str) -> io::Result<libc::stat> {
    let c_path = to_cstring(path)?;
    let mut buf = std::mem::MaybeUninit::<libc::stat>::zeroed();
    let result = (syscalls().lstat)(c_path.as_ptr(), buf.as_mut_ptr());
    if result == 0 {
        Ok(unsafe { buf.assume_init() })
    } else {
        Err(io::Error::last_os_error())
    }
}

pub fn open_file(path: &str, flags: c_int, mode: mode_t) -> io::Result<c_int> {
    let c_path = to_cstring(path)?;
    let result = (syscalls().open)(c_path.as_ptr(), flags, mode);
    if result >= 0 {
        Ok(result)
    } else {
        Err(io::Error::last_os_error())
    }
}

pub fn write_fd(fd: c_int, data: &[u8]) -> io::Result<usize> {
    let result = (syscalls().write)(fd, data.as_ptr(), data.len());
    if result >= 0 {
        Ok(result as usize)
    } else {
        Err(io::Error::last_os_error())
    }
}

pub fn write_all_fd(fd: c_int, mut data: &[u8]) -> io::Result<()> {
    while !data.is_empty() {
        let n = write_fd(fd, data)?;
        if n == 0 {
            return Err(io::Error::new(io::ErrorKind::WriteZero, "write returned 0"));
        }
        data = &data[n..];
    }
    Ok(())
}

pub fn stat_path(path: &str) -> io::Result<FileStat> {
    let raw = stat_raw(path)?;
    Ok(FileStat {
        mode: raw.st_mode,
        size: raw.st_size as u64,
    })
}

pub fn lstat_path(path: &str) -> io::Result<FileStat> {
    let raw = lstat_raw(path)?;
    Ok(FileStat {
        mode: raw.st_mode,
        size: raw.st_size as u64,
    })
}

pub fn access_path(path: &str, mode: c_int) -> io::Result<()> {
    let c_path = to_cstring(path)?;
    let result = (syscalls().access)(c_path.as_ptr(), mode);
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

pub fn file_exists(path: &str) -> bool {
    access_path(path, F_OK).is_ok()
}

pub fn is_directory(path: &str) -> bool {
    stat_path(path).map(|s| s.is_dir()).unwrap_or(false)
}

pub fn is_regular_file(path: &str) -> bool {
    stat_path(path).map(|s| s.is_regular_file()).unwrap_or(false)
}

pub fn change_dir(path: &str) -> io::Result<()> {
    let c_path = to_cstring(path)?;
    let result = (syscalls().chdir)(c_path.as_ptr());
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

pub fn get_cwd() -> io::Result<String> {
    let mut buf = vec![0u8; 4096];
    let result = (syscalls().getcwd)(buf.as_mut_ptr().cast(), buf.len());
    if result.is_null() {
        Err(io::Error::last_os_error())
    } else {
        let cstr = unsafe { CStr::from_ptr(result) };
        Ok(cstr.to_string_lossy().into_owned())
    }
}

pub fn read_dir_entries(path: &str) -> io::Result<Vec<String>> {
    let c_path = to_cstring(path)?;
    let dirp = (syscalls().opendir)(c_path.as_ptr());
    if dirp.is_null() {
        return Err(io::Error::last_os_error());
    }

    let mut entries = Vec::new();
    loop {
        // Clear errno before readdir; null return with errno=0 means end of directory
        unsafe { *libc::__error() = 0 };
        let ent = (syscalls().readdir)(dirp);
        if ent.is_null() {
            let errno = io::Error::last_os_error();
            (syscalls().closedir)(dirp);
            if errno.raw_os_error() == Some(0) {
                break;
            }
            return Err(errno);
        }
        let name = unsafe { CStr::from_ptr((*ent).d_name.as_ptr()) };
        let name = name.to_string_lossy().into_owned();
        if name != "." && name != ".." {
            entries.push(name);
        }
    }
    Ok(entries)
}

pub fn canonicalize(path: &str) -> io::Result<String> {
    let c_path = to_cstring(path)?;
    let result = (syscalls().realpath)(c_path.as_ptr(), std::ptr::null_mut());
    if result.is_null() {
        Err(io::Error::last_os_error())
    } else {
        let s = unsafe { CStr::from_ptr(result) }.to_string_lossy().into_owned();
        unsafe { libc::free(result.cast()) };
        Ok(s)
    }
}

pub fn read_link(path: &str) -> io::Result<String> {
    let c_path = to_cstring(path)?;
    let mut buf = vec![0u8; 4096];
    let result = (syscalls().readlink)(c_path.as_ptr(), buf.as_mut_ptr().cast(), buf.len());
    if result < 0 {
        Err(io::Error::last_os_error())
    } else {
        buf.truncate(result as usize);
        Ok(String::from_utf8_lossy(&buf).into_owned())
    }
}

pub fn unlink_file(path: &str) -> io::Result<()> {
    let c_path = to_cstring(path)?;
    let result = (syscalls().unlink)(c_path.as_ptr());
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

pub fn read_file(path: &str) -> io::Result<String> {
    let fd = open_file(path, O_RDONLY | O_CLOEXEC, 0)?;
    let mut contents = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = read_fd(fd, &mut buf)?;
        if n == 0 {
            break;
        }
        contents.extend_from_slice(&buf[..n]);
    }
    close_fd(fd)?;
    Ok(String::from_utf8_lossy(&contents).into_owned())
}

pub fn open_for_redirect(path: &str, flags: c_int, mode: mode_t, noclobber: bool) -> io::Result<c_int> {
    let actual_flags = if noclobber && (flags & O_TRUNC != 0) {
        (flags & !O_TRUNC) | O_EXCL | O_CREAT
    } else {
        flags
    };
    open_file(path, actual_flags, mode)
}

// --- Process wrapper functions ---

pub struct ChildHandle {
    pub pid: Pid,
    pub stdout_fd: Option<c_int>,
}

pub struct ChildOutput {
    pub status: ChildExitStatus,
    pub stdout: Vec<u8>,
}

#[derive(Clone, Copy, Debug)]
pub struct ChildExitStatus {
    code: i32,
}

impl ChildExitStatus {
    pub fn success(&self) -> bool {
        self.code == 0
    }

    pub fn code(&self) -> Option<i32> {
        Some(self.code)
    }
}

impl ChildHandle {
    pub fn wait_with_output(self) -> io::Result<ChildOutput> {
        let mut output = Vec::new();
        if let Some(fd) = self.stdout_fd {
            let mut buf = [0u8; 8192];
            loop {
                let n = read_fd(fd, &mut buf)?;
                if n == 0 {
                    break;
                }
                output.extend_from_slice(&buf[..n]);
            }
            close_fd(fd)?;
        }
        let ws = wait_pid(self.pid, false)?.expect("child status");
        Ok(ChildOutput {
            status: ChildExitStatus { code: decode_wait_status(ws.status) },
            stdout: output,
        })
    }

    pub fn wait(self) -> io::Result<ChildExitStatus> {
        if let Some(fd) = self.stdout_fd {
            close_fd(fd)?;
        }
        let ws = wait_pid(self.pid, false)?.expect("child status");
        Ok(ChildExitStatus { code: decode_wait_status(ws.status) })
    }
}

pub fn fork_process() -> io::Result<Pid> {
    let pid = (syscalls().fork)();
    if pid < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(pid)
    }
}

pub fn spawn_child(
    program: &str,
    argv: &[&str],
    env_vars: Option<&[(&str, &str)]>,
    redirections: &[(c_int, c_int)],
    stdin_fd: Option<c_int>,
    pipe_stdout: bool,
    process_group: Option<Pid>,
) -> io::Result<ChildHandle> {
    let stdout_pipe = if pipe_stdout {
        let (r, w) = create_pipe()?;
        Some((r, w))
    } else {
        None
    };

    let pid = fork_process()?;
    if pid == 0 {
        // Child process
        if let Some(pgid) = process_group {
            let _ = set_process_group(0, pgid);
        }
        if let Some(fd) = stdin_fd {
            let _ = duplicate_fd(fd, STDIN_FILENO);
            let _ = close_fd(fd);
        }
        if let Some((r, w)) = stdout_pipe {
            let _ = close_fd(r);
            let _ = duplicate_fd(w, STDOUT_FILENO);
            let _ = close_fd(w);
        }
        for &(src, dst) in redirections {
            let _ = duplicate_fd(src, dst);
            if src != dst {
                let _ = close_fd(src);
            }
        }
        if let Some(vars) = env_vars {
            for &(key, value) in vars {
                let kv = format!("{key}={value}");
                if let Ok(c_kv) = CString::new(kv) {
                    unsafe { libc::putenv(c_kv.into_raw()) };
                }
            }
        }
        let rest: Vec<String> = argv.get(1..).unwrap_or(&[]).iter().map(|s| s.to_string()).collect();
        let _ = exec_replace(program, &rest);
        unsafe { libc::_exit(127) };
    }

    // Parent process
    if let Some(fd) = stdin_fd {
        let _ = close_fd(fd);
    }
    let stdout_read = if let Some((r, w)) = stdout_pipe {
        let _ = close_fd(w);
        Some(r)
    } else {
        None
    };
    if let Some(pgid) = process_group {
        let _ = set_process_group(pid, pgid);
    }

    Ok(ChildHandle { pid, stdout_fd: stdout_read })
}

pub fn capture_child_output(
    program: &str,
    argv: &[&str],
    env_vars: Option<&[(&str, &str)]>,
) -> io::Result<(i32, Vec<u8>)> {
    let handle = spawn_child(program, argv, env_vars, &[], None, true, None)?;
    let stdout_fd = handle.stdout_fd.expect("piped stdout");
    let mut output = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = read_fd(stdout_fd, &mut buf)?;
        if n == 0 {
            break;
        }
        output.extend_from_slice(&buf[..n]);
    }
    close_fd(stdout_fd)?;
    let ws = wait_pid(handle.pid, false)?.expect("child status");
    Ok((decode_wait_status(ws.status), output))
}

pub fn run_to_status(
    program: &str,
    argv: &[&str],
    env_vars: Option<&[(&str, &str)]>,
) -> io::Result<i32> {
    let handle = spawn_child(program, argv, env_vars, &[], None, false, None)?;
    let ws = wait_pid(handle.pid, false)?.expect("child status");
    Ok(decode_wait_status(ws.status))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcessTimes {
    pub user_ticks: u64,
    pub system_ticks: u64,
    pub child_user_ticks: u64,
    pub child_system_ticks: u64,
}

pub fn current_umask() -> FileModeMask {
    let mask = (syscalls().umask)(0);
    (syscalls().umask)(mask);
    mask & 0o777
}

pub fn set_umask(mask: FileModeMask) -> FileModeMask {
    (syscalls().umask)(mask & 0o777) & 0o777
}

pub fn process_times() -> io::Result<ProcessTimes> {
    let mut raw = std::mem::MaybeUninit::<libc::tms>::zeroed();
    let result = (syscalls().times)(raw.as_mut_ptr());
    if result == ClockTicks::MAX {
        return Err(io::Error::last_os_error());
    }
    let raw = unsafe { raw.assume_init() };
    Ok(ProcessTimes {
        user_ticks: raw.tms_utime as u64,
        system_ticks: raw.tms_stime as u64,
        child_user_ticks: raw.tms_cutime as u64,
        child_system_ticks: raw.tms_cstime as u64,
    })
}

pub fn clock_ticks_per_second() -> io::Result<u64> {
    let result = (syscalls().sysconf)(SC_CLK_TCK);
    if result > 0 {
        Ok(result as u64)
    } else {
        Err(io::Error::last_os_error())
    }
}

/// Execute a program, replacing the current process image.
/// `program` is the file to exec and becomes argv[0].
/// `argv` contains the remaining arguments (argv[1..]).
pub fn exec_replace(program: &str, argv: &[String]) -> io::Result<()> {
    let mut owned = Vec::with_capacity(argv.len() + 1);
    owned.push(CString::new(program)?);
    for arg in argv {
        owned.push(CString::new(arg.as_str())?);
    }

    let mut pointers: Vec<*const c_char> = owned.iter().map(|arg| arg.as_ptr()).collect();
    pointers.push(std::ptr::null());

    let result = (syscalls().execvp)(owned[0].as_ptr(), pointers.as_ptr());
    if result == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub fn decode_wait_status(status: c_int) -> i32 {
    if wifexited(status) {
        wexitstatus(status)
    } else if wifsignaled(status) {
        128 + wtermsig(status)
    } else {
        status
    }
}

pub fn format_signal_exit(status: c_int) -> Option<String> {
    if wifsignaled(status) {
        Some(format!("terminated by signal {}", wtermsig(status)))
    } else {
        None
    }
}

fn signal_mask(signal: c_int) -> Option<usize> {
    let bit = match signal {
        SIGHUP => 0,
        SIGINT => 1,
        SIGQUIT => 2,
        SIGABRT => 3,
        SIGALRM => 4,
        SIGTERM => 5,
        _ => return None,
    };
    Some(1usize << bit)
}

fn wifexited(status: c_int) -> bool {
    (status & 0x7f) == 0
}

fn wexitstatus(status: c_int) -> i32 {
    (status >> 8) & 0xff
}

fn wifsignaled(status: c_int) -> bool {
    (status & 0x7f) != 0 && (status & 0x7f) != 0x7f
}

fn wtermsig(status: c_int) -> i32 {
    status & 0x7f
}

pub fn shell_name_from_args(args: &[String]) -> &str {
    args.first().map(String::as_str).unwrap_or("meiksh")
}

pub fn cstr_lossy(bytes: &[u8]) -> String {
    CStr::from_bytes_until_nul(bytes)
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|_| String::from_utf8_lossy(bytes).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipe_roundtrip() {
        fn fake_pipe(fds: *mut c_int) -> c_int {
            unsafe { *fds.add(0) = 10; *fds.add(1) = 11; }
            0
        }
        fn fake_close(_fd: c_int) -> c_int { 0 }

        let fake = Syscalls { pipe: fake_pipe, close: fake_close, ..default_syscalls() };
        test_support::with_test_syscalls(fake, || {
            let (read_fd, write_fd) = create_pipe().expect("pipe");
            assert_eq!(read_fd, 10);
            assert_eq!(write_fd, 11);
            close_fd(read_fd).expect("close read");
            close_fd(write_fd).expect("close write");
        });
    }

    #[test]
    fn decodes_wait_status_shapes() {
        assert_eq!(decode_wait_status(0), 0);
        assert_eq!(decode_wait_status(7 << 8), 7);
        assert_eq!(format_signal_exit(9), Some("terminated by signal 9".to_string()));
        assert_eq!(format_signal_exit(0), None);
    }

    #[test]
    fn shell_name_and_cstr_helpers_work() {
        assert_eq!(shell_name_from_args(&["meiksh".to_string(), "-c".to_string()]), "meiksh");
        assert_eq!(shell_name_from_args(&[]), "meiksh");
        assert_eq!(cstr_lossy(b"abc\0rest"), "abc".to_string());
        assert_eq!(cstr_lossy(b"plain-bytes"), "plain-bytes".to_string());

        fn fail_execvp(_file: *const c_char, _argv: *const *const c_char) -> c_int { -1 }
        let fake = Syscalls { execvp: fail_execvp, ..default_syscalls() };
        test_support::with_test_syscalls(fake, || {
            let program = CString::new("meiksh-command-that-does-not-exist").expect("cstring");
            let argv = [program.as_ptr(), std::ptr::null()];
            assert_eq!((syscalls().execvp)(program.as_ptr(), argv.as_ptr()), -1);
        });
    }

    #[test]
    fn invalid_fd_operations_fail_cleanly() {
        fn fail_isatty(_fd: c_int) -> c_int { 0 }
        fn fail_dup2(_old: c_int, _new: c_int) -> c_int { -1 }
        fn fail_close(_fd: c_int) -> c_int { -1 }
        fn fail_tcgetpgrp(_fd: c_int) -> Pid { -1 }
        fn fail_tcsetpgrp(_fd: c_int, _pgid: Pid) -> c_int { -1 }
        fn fail_setpgid(_pid: Pid, _pgid: Pid) -> c_int { -1 }

        let fake = Syscalls {
            isatty: fail_isatty, dup2: fail_dup2, close: fail_close,
            tcgetpgrp: fail_tcgetpgrp, tcsetpgrp: fail_tcsetpgrp, setpgid: fail_setpgid,
            ..default_syscalls()
        };
        test_support::with_test_syscalls(fake, || {
            assert!(!is_interactive_fd(-1));
            assert!(duplicate_fd(-1, -1).is_err());
            assert!(close_fd(-1).is_err());
            assert!(current_foreground_pgrp(-1).is_err());
            assert!(set_foreground_pgrp(-1, 0).is_err());
            assert!(set_process_group(999_999, 999_999).is_err());
        });
    }

    #[test]
    fn wait_pid_error_and_exec_replace_nul_error_work() {
        fn fail_waitpid(_pid: Pid, _status: *mut c_int, _options: c_int) -> Pid { -1 }
        let fake = Syscalls { waitpid: fail_waitpid, ..default_syscalls() };
        test_support::with_test_syscalls(fake, || {
            assert!(wait_pid(999_999, false).is_err());
        });
        assert!(exec_replace("bad\0program", &["bad\0program".to_string()]).is_err());
    }

    #[test]
    fn sys_success_branches_cover_fd_helpers() {
        fn fake_pipe(fds: *mut c_int) -> c_int {
            unsafe { *fds.add(0) = 20; *fds.add(1) = 21; }
            0
        }
        fn fake_dup2(oldfd: c_int, _newfd: c_int) -> c_int { oldfd }
        fn fake_close(_fd: c_int) -> c_int { 0 }

        let fake = Syscalls { pipe: fake_pipe, dup2: fake_dup2, close: fake_close, ..default_syscalls() };
        test_support::with_test_syscalls(fake, || {
            let (read_fd, write_fd) = create_pipe().expect("pipe");
            duplicate_fd(read_fd, read_fd).expect("dup self");
            close_fd(read_fd).expect("close read");
            close_fd(write_fd).expect("close write");
        });
    }

    #[test]
    fn process_identity_helper_covers_mismatch_branch() {
        assert!(!test_support::with_process_ids_for_test((1, 2, 3, 3), has_same_real_and_effective_ids));
        assert!(!test_support::with_process_ids_for_test((1, 1, 3, 4), has_same_real_and_effective_ids));
    }

    #[test]
    fn sys_injected_success_paths_cover_remaining_branches() {
        fn fake_getpid() -> Pid {
            4242
        }
        fn fake_waitpid(_pid: Pid, status: *mut c_int, _options: c_int) -> Pid {
            unsafe {
                *status = 9 << 8;
            }
            99
        }
        fn fake_kill(_pid: Pid, _sig: c_int) -> c_int {
            0
        }
        fn fake_signal(_sig: c_int, _handler: libc::sighandler_t) -> libc::sighandler_t {
            0
        }
        fn fake_isatty(_fd: c_int) -> c_int {
            1
        }
        fn fake_tcgetpgrp(_fd: c_int) -> Pid {
            77
        }
        fn fake_tcsetpgrp(_fd: c_int, _pgrp: Pid) -> c_int {
            0
        }
        fn fake_setpgid(_pid: Pid, _pgid: Pid) -> c_int {
            0
        }
        fn fake_pipe(fds: *mut c_int) -> c_int {
            unsafe {
                *fds.add(0) = 10;
                *fds.add(1) = 11;
            }
            0
        }
        fn fake_dup(fd: c_int) -> c_int {
            fd + 100
        }
        fn fake_dup2(oldfd: c_int, _newfd: c_int) -> c_int {
            oldfd
        }
        fn fake_close(_fd: c_int) -> c_int {
            0
        }
        fn fake_fcntl(_fd: c_int, cmd: c_int, arg: c_int) -> c_int {
            match cmd {
                F_GETFL => arg,
                F_SETFL => 0,
                _ => -1,
            }
        }
        fn fake_read(_fd: c_int, buf: *mut u8, count: usize) -> isize {
            if count == 0 {
                return 0;
            }
            unsafe {
                *buf = b'X';
            }
            1
        }
        fn fake_umask(mask: FileModeMask) -> FileModeMask {
            mask
        }
        fn fake_times(buffer: *mut libc::tms) -> ClockTicks {
            unsafe {
                (*buffer).tms_utime = 10;
                (*buffer).tms_stime = 20;
                (*buffer).tms_cutime = 30;
                (*buffer).tms_cstime = 40;
            }
            99
        }
        fn fake_sysconf(_name: c_int) -> c_long {
            60
        }
        fn fake_execvp(_file: *const c_char, _argv: *const *const c_char) -> c_int {
            0
        }

        let fake = Syscalls {
            getpid: fake_getpid,
            waitpid: fake_waitpid,
            kill: fake_kill,
            signal: fake_signal,
            isatty: fake_isatty,
            tcgetpgrp: fake_tcgetpgrp,
            tcsetpgrp: fake_tcsetpgrp,
            setpgid: fake_setpgid,
            pipe: fake_pipe,
            dup: fake_dup,
            dup2: fake_dup2,
            close: fake_close,
            fcntl: fake_fcntl,
            read: fake_read,
            umask: fake_umask,
            times: fake_times,
            sysconf: fake_sysconf,
            execvp: fake_execvp,
            ..default_syscalls()
        };

        test_support::with_test_syscalls(fake, || {
            assert_eq!(current_pid(), 4242);
            assert!(is_interactive_fd(0));
            assert_eq!(
                wait_pid(1, false).expect("wait").expect("status"),
                WaitStatus { pid: 99, status: 9 << 8 }
            );
            assert!(send_signal(1, 0).is_ok());
            assert_eq!(current_foreground_pgrp(0).expect("pgrp"), 77);
            assert!(set_foreground_pgrp(0, 77).is_ok());
            assert!(set_process_group(1, 1).is_ok());
            assert_eq!(create_pipe().expect("pipe"), (10, 11));
            assert_eq!(duplicate_fd_to_new(4).expect("dup"), 104);
            assert!(duplicate_fd(4, 5).is_ok());
            assert!(close_fd(4).is_ok());
            let mut buffer = [0u8; 1];
            assert_eq!(read_fd(0, &mut buffer).expect("read"), 1);
            assert_eq!(buffer, [b'X']);
            let mut reader = FdReader::new(0);
            assert_eq!(reader.read(&mut buffer).expect("reader read"), 1);
            assert_eq!(buffer, [b'X']);
            assert_eq!(current_umask(), 0);
            assert_eq!(set_umask(0o027), 0o027);
            assert_eq!(
                process_times().expect("times"),
                ProcessTimes {
                    user_ticks: 10,
                    system_ticks: 20,
                    child_user_ticks: 30,
                    child_system_ticks: 40,
                }
            );
            assert_eq!(clock_ticks_per_second().expect("ticks"), 60);
            assert!(exec_replace("echo", &["hello".to_string(), "world".to_string()]).is_ok());
        });
    }

    #[test]
    fn decode_wait_status_covers_fallback_shape() {
        assert_eq!(decode_wait_status(0x7f), 0x7f);
    }

    #[test]
    fn signal_helpers_cover_pending_ignore_default_and_error_paths() {
        fn ok_signal(_sig: c_int, _handler: libc::sighandler_t) -> libc::sighandler_t {
            0
        }
        fn err_signal(_sig: c_int, _handler: libc::sighandler_t) -> libc::sighandler_t {
            SIG_ERR_HANDLER
        }

        test_support::with_signal_syscall_for_test(ok_signal, || {
            install_shell_signal_handler(SIGINT).expect("install");
            ignore_signal(SIGTERM).expect("ignore");
            default_signal_action(SIGQUIT).expect("default");
        });

        test_support::with_signal_syscall_for_test(err_signal, || {
            assert!(install_shell_signal_handler(SIGINT).is_err());
            assert!(ignore_signal(SIGTERM).is_err());
            assert!(default_signal_action(SIGQUIT).is_err());
        });

        test_support::with_pending_signals_for_test(&[SIGINT], || {
            assert_eq!(has_pending_signal(), Some(SIGINT));
            assert_eq!(take_pending_signals(), vec![SIGINT]);
        });
        test_support::with_pending_signals_for_test(&[99], || {
            assert_eq!(has_pending_signal(), None);
        });

        let interrupted_error = io::Error::from_raw_os_error(EINTR);
        assert!(interrupted(&interrupted_error));
        assert_eq!(supported_trap_signals(), vec![SIGHUP, SIGINT, SIGQUIT, SIGABRT, SIGALRM, SIGTERM]);
    }

    #[test]
    fn sys_injected_error_paths_cover_remaining_branches() {
        fn fake_getpid() -> Pid {
            1
        }
        fn fake_waitpid(_pid: Pid, _status: *mut c_int, _options: c_int) -> Pid {
            -1
        }
        fn fake_kill(_pid: Pid, _sig: c_int) -> c_int {
            -1
        }
        fn fake_signal(_sig: c_int, _handler: libc::sighandler_t) -> libc::sighandler_t {
            SIG_ERR_HANDLER
        }
        fn fake_isatty(_fd: c_int) -> c_int {
            0
        }
        fn fake_tcgetpgrp(_fd: c_int) -> Pid {
            -1
        }
        fn fake_tcsetpgrp(_fd: c_int, _pgrp: Pid) -> c_int {
            -1
        }
        fn fake_setpgid(_pid: Pid, _pgid: Pid) -> c_int {
            -1
        }
        fn fake_pipe(_fds: *mut c_int) -> c_int {
            -1
        }
        fn fake_dup(_fd: c_int) -> c_int {
            -1
        }
        fn fake_dup2(_oldfd: c_int, _newfd: c_int) -> c_int {
            -1
        }
        fn fake_close(_fd: c_int) -> c_int {
            -1
        }
        fn fake_fcntl(_fd: c_int, _cmd: c_int, _arg: c_int) -> c_int {
            -1
        }
        fn fake_read(_fd: c_int, _buf: *mut u8, _count: usize) -> isize {
            -1
        }
        fn fake_umask(mask: FileModeMask) -> FileModeMask {
            mask
        }
        fn fake_times(_buffer: *mut libc::tms) -> ClockTicks {
            ClockTicks::MAX
        }
        fn fake_sysconf(_name: c_int) -> c_long {
            -1
        }
        fn fake_execvp(_file: *const c_char, _argv: *const *const c_char) -> c_int {
            -1
        }

        let fake = Syscalls {
            getpid: fake_getpid,
            waitpid: fake_waitpid,
            kill: fake_kill,
            signal: fake_signal,
            isatty: fake_isatty,
            tcgetpgrp: fake_tcgetpgrp,
            tcsetpgrp: fake_tcsetpgrp,
            setpgid: fake_setpgid,
            pipe: fake_pipe,
            dup: fake_dup,
            dup2: fake_dup2,
            close: fake_close,
            fcntl: fake_fcntl,
            read: fake_read,
            umask: fake_umask,
            times: fake_times,
            sysconf: fake_sysconf,
            execvp: fake_execvp,
            ..default_syscalls()
        };

        test_support::with_test_syscalls(fake, || {
            assert_eq!(current_pid(), 1);
            assert!(!is_interactive_fd(0));
            assert!(send_signal(1, 0).is_err());
            assert!(current_foreground_pgrp(0).is_err());
            assert!(set_foreground_pgrp(0, 1).is_err());
            assert!(set_process_group(1, 1).is_err());
            assert!(create_pipe().is_err());
            assert!(duplicate_fd_to_new(1).is_err());
            assert!(duplicate_fd(1, 2).is_err());
            assert!(close_fd(1).is_err());
            assert!(read_fd(0, &mut [0u8; 1]).is_err());
            assert!(process_times().is_err());
            assert!(clock_ticks_per_second().is_err());
            assert!(exec_replace("echo", &["hi".to_string()]).is_err());
            assert!(wait_pid(1, false).is_err());
        });

        assert_eq!(decode_wait_status(9), 137);
    }

    #[test]
    fn ensure_blocking_read_fd_clears_nonblocking_for_ttys_and_fifos() {
        static LAST_SET_FLAGS: AtomicUsize = AtomicUsize::new(usize::MAX);

        fn fake_isatty(_fd: c_int) -> c_int {
            1
        }
        fn fake_fcntl(_fd: c_int, cmd: c_int, flags: c_int) -> c_int {
            match cmd {
                F_GETFL => O_NONBLOCK | 0o2,
                F_SETFL => {
                    LAST_SET_FLAGS.store(flags as usize, Ordering::SeqCst);
                    0
                }
                _ => -1,
            }
        }

        test_support::with_fcntl_and_isatty_for_test(fake_fcntl, fake_isatty, || {
            LAST_SET_FLAGS.store(usize::MAX, Ordering::SeqCst);
            ensure_blocking_read_fd(STDIN_FILENO).expect("tty blocking");
            assert_eq!(LAST_SET_FLAGS.load(Ordering::SeqCst), 0o2);
        });

        fn not_tty(_fd: c_int) -> c_int {
            0
        }
        fn fake_fstat_fifo(_fd: c_int, buf: *mut libc::stat) -> c_int {
            unsafe { std::ptr::write_bytes(buf, 0, 1); (*buf).st_mode = libc::S_IFIFO; }
            0
        }

        let fake = Syscalls { fcntl: fake_fcntl, isatty: not_tty, fstat: fake_fstat_fifo, ..default_syscalls() };
        test_support::with_test_syscalls(fake, || {
            LAST_SET_FLAGS.store(usize::MAX, Ordering::SeqCst);
            ensure_blocking_read_fd(42).expect("fifo blocking");
            assert_eq!(LAST_SET_FLAGS.load(Ordering::SeqCst), 0o2);
        });
    }

    #[test]
    fn ensure_blocking_read_fd_skips_regular_files_and_surfaces_fcntl_errors() {
        static FCNTL_CALLS: AtomicUsize = AtomicUsize::new(0);

        fn not_tty(_fd: c_int) -> c_int {
            0
        }
        fn counting_fcntl(_fd: c_int, _cmd: c_int, _flags: c_int) -> c_int {
            FCNTL_CALLS.fetch_add(1, Ordering::SeqCst);
            0
        }
        fn failing_fcntl(_fd: c_int, _cmd: c_int, _flags: c_int) -> c_int {
            -1
        }

        test_support::VfsBuilder::new()
            .file("/tmp/regular.txt", b"x")
            .run_with_fcntl_and_isatty(counting_fcntl, not_tty, || {
                let fd = open_file("/tmp/regular.txt", O_RDONLY, 0).expect("open vfs file");
                FCNTL_CALLS.store(0, Ordering::SeqCst);
                ensure_blocking_read_fd(fd).expect("regular file");
                assert_eq!(FCNTL_CALLS.load(Ordering::SeqCst), 0);
                close_fd(fd).expect("close vfs fd");
            });

        test_support::with_fcntl_and_isatty_for_test(failing_fcntl, |_| 1, || {
            assert!(ensure_blocking_read_fd(STDIN_FILENO).is_err());
        });
    }
}
