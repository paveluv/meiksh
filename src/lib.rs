#![warn(clippy::disallowed_types)]
#![warn(clippy::disallowed_methods)]
#![warn(clippy::disallowed_macros)]

#[macro_use]
pub mod sys;
pub mod arena;
pub mod bstr;
pub mod builtin;
pub mod exec;
pub mod expand;
pub mod interactive;
pub mod shell;
pub mod syntax;

pub use shell::run_from_env;

#[cfg(test)]
#[allow(unused_macros)]
macro_rules! syscall_test {
    (
        name: $name:ident,
        args: [$($arg:expr),* $(,)?],
        trace: [$($trace:tt)*] $(,)?
    ) => {
        #[test]
        fn $name() {
            let trace = $crate::syscall_test!(@trace_entries $($trace)*);
            $crate::sys::test_support::run_trace(trace, || {
                let mut shell = $crate::shell::Shell::from_args(
                    &["meiksh", $($arg),*]
                ).expect("Shell::from_args");
                let _ = shell.run();
            });
        }
    };

    // Parse trace entries (comma-separated, trailing comma optional)
    (@trace_entries) => { vec![] };
    (@trace_entries $($entries:tt)+) => {{
        let mut trace: Vec<$crate::sys::test_support::TraceEntry> = Vec::new();
        $crate::syscall_test!(@parse_entries trace; $($entries)*);
        trace
    }};

    // Terminal case — no more tokens
    (@parse_entries $trace:ident;) => {};

    // spread a Vec<TraceEntry> from an expression
    (@parse_entries $trace:ident; ..$spread:expr, $($rest:tt)*) => {
        $trace.extend($spread);
        $crate::syscall_test!(@parse_entries $trace; $($rest)*);
    };
    (@parse_entries $trace:ident; ..$spread:expr) => {
        $trace.extend($spread);
    };

    // fork with child trace
    (@parse_entries $trace:ident; fork() -> pid($pid:expr), child: [$($child:tt)*], $($rest:tt)*) => {
        $trace.push($crate::sys::test_support::t_fork(
            $crate::sys::test_support::TraceResult::Pid($pid),
            $crate::syscall_test!(@trace_entries $($child)*),
        ));
        $crate::syscall_test!(@parse_entries $trace; $($rest)*);
    };
    (@parse_entries $trace:ident; fork() -> pid($pid:expr), child: [$($child:tt)*]) => {
        $trace.push($crate::sys::test_support::t_fork(
            $crate::sys::test_support::TraceResult::Pid($pid),
            $crate::syscall_test!(@trace_entries $($child)*),
        ));
    };

    // waitpid with status result
    (@parse_entries $trace:ident; waitpid($pid:expr, _) -> status($status:expr), $($rest:tt)*) => {
        $trace.push($crate::sys::test_support::t(
            "waitpid",
            vec![$crate::sys::test_support::ArgMatcher::Int($pid as i64), $crate::sys::test_support::ArgMatcher::Any, $crate::sys::test_support::ArgMatcher::Any],
            $crate::sys::test_support::TraceResult::Status($status),
        ));
        $crate::syscall_test!(@parse_entries $trace; $($rest)*);
    };
    (@parse_entries $trace:ident; waitpid($pid:expr, _) -> status($status:expr)) => {
        $trace.push($crate::sys::test_support::t(
            "waitpid",
            vec![$crate::sys::test_support::ArgMatcher::Int($pid as i64), $crate::sys::test_support::ArgMatcher::Any, $crate::sys::test_support::ArgMatcher::Any],
            $crate::sys::test_support::TraceResult::Status($status),
        ));
    };
    (@parse_entries $trace:ident; waitpid($pid:expr, _) -> stopped_sig($sig:expr), $($rest:tt)*) => {
        $trace.push($crate::sys::test_support::t(
            "waitpid",
            vec![$crate::sys::test_support::ArgMatcher::Int($pid as i64), $crate::sys::test_support::ArgMatcher::Any, $crate::sys::test_support::ArgMatcher::Any],
            $crate::sys::test_support::TraceResult::StoppedSig($sig),
        ));
        $crate::syscall_test!(@parse_entries $trace; $($rest)*);
    };
    (@parse_entries $trace:ident; waitpid($pid:expr, _) -> stopped_sig($sig:expr)) => {
        $trace.push($crate::sys::test_support::t(
            "waitpid",
            vec![$crate::sys::test_support::ArgMatcher::Int($pid as i64), $crate::sys::test_support::ArgMatcher::Any, $crate::sys::test_support::ArgMatcher::Any],
            $crate::sys::test_support::TraceResult::StoppedSig($sig),
        ));
    };
    (@parse_entries $trace:ident; waitpid($pid:expr, _) -> signaled_sig($sig:expr), $($rest:tt)*) => {
        $trace.push($crate::sys::test_support::t(
            "waitpid",
            vec![$crate::sys::test_support::ArgMatcher::Int($pid as i64), $crate::sys::test_support::ArgMatcher::Any, $crate::sys::test_support::ArgMatcher::Any],
            $crate::sys::test_support::TraceResult::SignaledSig($sig),
        ));
        $crate::syscall_test!(@parse_entries $trace; $($rest)*);
    };
    (@parse_entries $trace:ident; waitpid($pid:expr, _) -> signaled_sig($sig:expr)) => {
        $trace.push($crate::sys::test_support::t(
            "waitpid",
            vec![$crate::sys::test_support::ArgMatcher::Int($pid as i64), $crate::sys::test_support::ArgMatcher::Any, $crate::sys::test_support::ArgMatcher::Any],
            $crate::sys::test_support::TraceResult::SignaledSig($sig),
        ));
    };
    (@parse_entries $trace:ident; waitpid($pid:expr, _) -> continued, $($rest:tt)*) => {
        $trace.push($crate::sys::test_support::t(
            "waitpid",
            vec![$crate::sys::test_support::ArgMatcher::Int($pid as i64), $crate::sys::test_support::ArgMatcher::Any, $crate::sys::test_support::ArgMatcher::Any],
            $crate::sys::test_support::TraceResult::ContinuedStatus,
        ));
        $crate::syscall_test!(@parse_entries $trace; $($rest)*);
    };
    (@parse_entries $trace:ident; waitpid($pid:expr, _) -> continued) => {
        $trace.push($crate::sys::test_support::t(
            "waitpid",
            vec![$crate::sys::test_support::ArgMatcher::Int($pid as i64), $crate::sys::test_support::ArgMatcher::Any, $crate::sys::test_support::ArgMatcher::Any],
            $crate::sys::test_support::TraceResult::ContinuedStatus,
        ));
    };

    // Generic syscall: name(args...) -> result
    (@parse_entries $trace:ident; $syscall:ident($($args:tt)*) -> $($result:tt)*) => {
        $crate::syscall_test!(@emit_entry $trace; $syscall; ($($args)*); $($result)*);
    };

    // Handle splitting result from rest (result , rest...)
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); err($errno:expr), $($rest:tt)*) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            $crate::syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::Err($errno),
        ));
        $crate::syscall_test!(@parse_entries $trace; $($rest)*);
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); err($errno:expr)) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            $crate::syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::Err($errno),
        ));
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); bytes($bytes:expr), $($rest:tt)*) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            $crate::syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::Bytes($bytes.to_vec()),
        ));
        $crate::syscall_test!(@parse_entries $trace; $($rest)*);
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); bytes($bytes:expr)) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            $crate::syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::Bytes($bytes.to_vec()),
        ));
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); fd($fd:expr), $($rest:tt)*) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            $crate::syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::Fd($fd),
        ));
        $crate::syscall_test!(@parse_entries $trace; $($rest)*);
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); fd($fd:expr)) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            $crate::syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::Fd($fd),
        ));
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); pid($pid:expr), $($rest:tt)*) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            $crate::syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::Pid($pid),
        ));
        $crate::syscall_test!(@parse_entries $trace; $($rest)*);
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); pid($pid:expr)) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            $crate::syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::Pid($pid),
        ));
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); fds($r:expr, $w:expr), $($rest:tt)*) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            $crate::syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::Fds($r, $w),
        ));
        $crate::syscall_test!(@parse_entries $trace; $($rest)*);
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); fds($r:expr, $w:expr)) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            $crate::syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::Fds($r, $w),
        ));
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); cwd($s:expr), $($rest:tt)*) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            $crate::syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::CwdBytes($s.as_bytes().to_vec()),
        ));
        $crate::syscall_test!(@parse_entries $trace; $($rest)*);
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); cwd($s:expr)) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            $crate::syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::CwdBytes($s.as_bytes().to_vec()),
        ));
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); realpath($s:expr), $($rest:tt)*) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            $crate::syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::RealpathBytes($s.as_bytes().to_vec()),
        ));
        $crate::syscall_test!(@parse_entries $trace; $($rest)*);
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); realpath($s:expr)) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            $crate::syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::RealpathBytes($s.as_bytes().to_vec()),
        ));
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); stat_dir, $($rest:tt)*) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            $crate::syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::StatDir,
        ));
        $crate::syscall_test!(@parse_entries $trace; $($rest)*);
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); stat_dir) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            $crate::syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::StatDir,
        ));
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); stat_fifo, $($rest:tt)*) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            $crate::syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::StatFifo,
        ));
        $crate::syscall_test!(@parse_entries $trace; $($rest)*);
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); stat_fifo) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            $crate::syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::StatFifo,
        ));
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); stat_file($mode:expr), $($rest:tt)*) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            $crate::syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::StatFile($mode),
        ));
        $crate::syscall_test!(@parse_entries $trace; $($rest)*);
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); stat_file($mode:expr)) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            $crate::syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::StatFile($mode),
        ));
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); stat_fifo, $($rest:tt)*) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            $crate::syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::StatFifo,
        ));
        $crate::syscall_test!(@parse_entries $trace; $($rest)*);
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); stat_fifo) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            $crate::syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::StatFifo,
        ));
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); stat_file_size($sz:expr), $($rest:tt)*) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            $crate::syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::StatFileSize($sz),
        ));
        $crate::syscall_test!(@parse_entries $trace; $($rest)*);
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); stat_file_size($sz:expr)) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            $crate::syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::StatFileSize($sz),
        ));
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); dir_entry($name:expr), $($rest:tt)*) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            $crate::syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::DirEntryBytes(
                $crate::sys::test_support::trace_bytes_from_ref(&($name)),
            ),
        ));
        $crate::syscall_test!(@parse_entries $trace; $($rest)*);
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); dir_entry($name:expr)) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            $crate::syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::DirEntryBytes(
                $crate::sys::test_support::trace_bytes_from_ref(&($name)),
            ),
        ));
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); status($s:expr), $($rest:tt)*) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            $crate::syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::Status($s),
        ));
        $crate::syscall_test!(@parse_entries $trace; $($rest)*);
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); status($s:expr)) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            $crate::syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::Status($s),
        ));
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); stopped_sig($sig:expr), $($rest:tt)*) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            $crate::syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::StoppedSig($sig),
        ));
        $crate::syscall_test!(@parse_entries $trace; $($rest)*);
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); stopped_sig($sig:expr)) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            $crate::syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::StoppedSig($sig),
        ));
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); signaled_sig($sig:expr), $($rest:tt)*) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            $crate::syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::SignaledSig($sig),
        ));
        $crate::syscall_test!(@parse_entries $trace; $($rest)*);
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); signaled_sig($sig:expr)) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            $crate::syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::SignaledSig($sig),
        ));
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); continued, $($rest:tt)*) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            $crate::syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::ContinuedStatus,
        ));
        $crate::syscall_test!(@parse_entries $trace; $($rest)*);
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); continued) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            $crate::syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::ContinuedStatus,
        ));
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); auto, $($rest:tt)*) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            $crate::syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::Auto,
        ));
        $crate::syscall_test!(@parse_entries $trace; $($rest)*);
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); auto) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            $crate::syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::Auto,
        ));
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); interrupt($sig:expr), $($rest:tt)*) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            $crate::syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::Interrupt($sig),
        ));
        $crate::syscall_test!(@parse_entries $trace; $($rest)*);
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); interrupt($sig:expr)) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            $crate::syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::Interrupt($sig),
        ));
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); int($v:expr), $($rest:tt)*) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            $crate::syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::Int($v as i64),
        ));
        $crate::syscall_test!(@parse_entries $trace; $($rest)*);
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); int($v:expr)) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            $crate::syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::Int($v as i64),
        ));
    };
    // Wildcard return
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); _, $($rest:tt)*) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            $crate::syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::Int(0),
        ));
        $crate::syscall_test!(@parse_entries $trace; $($rest)*);
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); _) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            $crate::syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::Int(0),
        ));
    };
    // Void result (for exit-like calls)
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); void, $($rest:tt)*) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            $crate::syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::Void,
        ));
        $crate::syscall_test!(@parse_entries $trace; $($rest)*);
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); void) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            $crate::syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::Void,
        ));
    };
    // Integer result
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); $val:expr, $($rest:tt)*) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            $crate::syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::Int($val as i64),
        ));
        $crate::syscall_test!(@parse_entries $trace; $($rest)*);
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); $val:expr) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            $crate::syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::Int($val as i64),
        ));
    };

    // Parse argument list
    (@args) => { vec![] };
    (@args _) => { vec![$crate::sys::test_support::ArgMatcher::Any] };
    (@args _, $($rest:tt)*) => {{
        let mut args = vec![$crate::sys::test_support::ArgMatcher::Any];
        args.extend($crate::syscall_test!(@args $($rest)*));
        args
    }};
    (@args any) => { vec![$crate::sys::test_support::ArgMatcher::Any] };
    (@args any, $($rest:tt)*) => {{
        let mut args = vec![$crate::sys::test_support::ArgMatcher::Any];
        args.extend($crate::syscall_test!(@args $($rest)*));
        args
    }};
    (@args int($e:expr)) => {
        vec![$crate::sys::test_support::ArgMatcher::Int($e as i64)]
    };
    (@args int($e:expr), $($rest:tt)*) => {{
        let mut args = vec![$crate::sys::test_support::ArgMatcher::Int($e as i64)];
        args.extend($crate::syscall_test!(@args $($rest)*));
        args
    }};
    (@args fd($e:expr)) => { vec![$crate::sys::test_support::ArgMatcher::Fd($e)] };
    (@args fd($e:expr), $($rest:tt)*) => {{
        let mut args = vec![$crate::sys::test_support::ArgMatcher::Fd($e)];
        args.extend($crate::syscall_test!(@args $($rest)*));
        args
    }};
    (@args bytes($e:expr)) => {
        vec![$crate::sys::test_support::ArgMatcher::Bytes(
            $crate::sys::test_support::trace_bytes_from_ref(&($e)),
        )]
    };
    (@args bytes($e:expr), $($rest:tt)*) => {{
        let mut args = vec![$crate::sys::test_support::ArgMatcher::Bytes(
            $crate::sys::test_support::trace_bytes_from_ref(&($e)),
        )];
        args.extend($crate::syscall_test!(@args $($rest)*));
        args
    }};
    (@args str($e:expr)) => {
        vec![$crate::sys::test_support::ArgMatcher::Str(
            $crate::sys::test_support::trace_str_from_ref(&($e)),
        )]
    };
    (@args str($e:expr), $($rest:tt)*) => {{
        let mut args = vec![$crate::sys::test_support::ArgMatcher::Str(
            $crate::sys::test_support::trace_str_from_ref(&($e)),
        )];
        args.extend($crate::syscall_test!(@args $($rest)*));
        args
    }};
    (@args $arg:expr) => { vec![$crate::syscall_test!(@one_arg $arg)] };
    (@args $arg:expr, $($rest:tt)*) => {{
        let mut args = vec![$crate::syscall_test!(@one_arg $arg)];
        args.extend($crate::syscall_test!(@args $($rest)*));
        args
    }};

    // Single argument conversion (expr path)
    (@one_arg _) => { $crate::sys::test_support::ArgMatcher::Any };
    (@one_arg any) => { $crate::sys::test_support::ArgMatcher::Any };
    (@one_arg $arg:expr) => { $crate::sys::test_support::arg_from($arg) };
}

#[cfg(test)]
#[allow(unused_macros)]
macro_rules! trace_entries {
    ($($entries:tt)*) => {{
        #[allow(unused_mut)]
        let mut trace: Vec<$crate::sys::test_support::TraceEntry> = Vec::new();
        $crate::syscall_test!(@parse_entries trace; $($entries)*);
        trace
    }};
}

#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use syscall_test;

#[cfg(test)]
pub(crate) use trace_entries;
