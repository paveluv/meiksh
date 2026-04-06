#![warn(clippy::disallowed_types)]
#![warn(clippy::disallowed_methods)]
#![warn(clippy::disallowed_macros)]

#[macro_use]
pub mod sys;
pub mod arena;
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
            let trace = syscall_test!(@trace_entries $($trace)*);
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
        syscall_test!(@parse_entries trace; $($entries)*);
        trace
    }};

    // Terminal case — no more tokens
    (@parse_entries $trace:ident;) => {};

    // spread a Vec<TraceEntry> from an expression
    (@parse_entries $trace:ident; ..$spread:expr, $($rest:tt)*) => {
        $trace.extend($spread);
        syscall_test!(@parse_entries $trace; $($rest)*);
    };
    (@parse_entries $trace:ident; ..$spread:expr) => {
        $trace.extend($spread);
    };

    // fork with child trace
    (@parse_entries $trace:ident; fork() -> pid($pid:expr), child: [$($child:tt)*], $($rest:tt)*) => {
        $trace.push($crate::sys::test_support::t_fork(
            $crate::sys::test_support::TraceResult::Pid($pid),
            syscall_test!(@trace_entries $($child)*),
        ));
        syscall_test!(@parse_entries $trace; $($rest)*);
    };
    (@parse_entries $trace:ident; fork() -> pid($pid:expr), child: [$($child:tt)*]) => {
        $trace.push($crate::sys::test_support::t_fork(
            $crate::sys::test_support::TraceResult::Pid($pid),
            syscall_test!(@trace_entries $($child)*),
        ));
    };

    // waitpid with status result
    (@parse_entries $trace:ident; waitpid($pid:expr, _) -> status($status:expr), $($rest:tt)*) => {
        $trace.push($crate::sys::test_support::t(
            "waitpid",
            vec![$crate::sys::test_support::ArgMatcher::Int($pid as i64), $crate::sys::test_support::ArgMatcher::Any, $crate::sys::test_support::ArgMatcher::Any],
            $crate::sys::test_support::TraceResult::Status($status),
        ));
        syscall_test!(@parse_entries $trace; $($rest)*);
    };
    (@parse_entries $trace:ident; waitpid($pid:expr, _) -> status($status:expr)) => {
        $trace.push($crate::sys::test_support::t(
            "waitpid",
            vec![$crate::sys::test_support::ArgMatcher::Int($pid as i64), $crate::sys::test_support::ArgMatcher::Any, $crate::sys::test_support::ArgMatcher::Any],
            $crate::sys::test_support::TraceResult::Status($status),
        ));
    };

    // Generic syscall: name(args...) -> result
    (@parse_entries $trace:ident; $syscall:ident($($args:tt)*) -> $($result:tt)*) => {
        syscall_test!(@emit_entry $trace; $syscall; ($($args)*); $($result)*);
    };

    // Handle splitting result from rest (result , rest...)
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); err($errno:expr), $($rest:tt)*) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::Err($errno),
        ));
        syscall_test!(@parse_entries $trace; $($rest)*);
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); err($errno:expr)) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::Err($errno),
        ));
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); bytes($bytes:expr), $($rest:tt)*) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::Bytes($bytes.to_vec()),
        ));
        syscall_test!(@parse_entries $trace; $($rest)*);
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); bytes($bytes:expr)) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::Bytes($bytes.to_vec()),
        ));
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); fd($fd:expr), $($rest:tt)*) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::Fd($fd),
        ));
        syscall_test!(@parse_entries $trace; $($rest)*);
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); fd($fd:expr)) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::Fd($fd),
        ));
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); pid($pid:expr), $($rest:tt)*) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::Pid($pid),
        ));
        syscall_test!(@parse_entries $trace; $($rest)*);
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); pid($pid:expr)) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::Pid($pid),
        ));
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); fds($r:expr, $w:expr), $($rest:tt)*) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::Fds($r, $w),
        ));
        syscall_test!(@parse_entries $trace; $($rest)*);
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); fds($r:expr, $w:expr)) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::Fds($r, $w),
        ));
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); cwd($s:expr), $($rest:tt)*) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::CwdStr($s.to_string()),
        ));
        syscall_test!(@parse_entries $trace; $($rest)*);
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); cwd($s:expr)) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::CwdStr($s.to_string()),
        ));
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); realpath($s:expr), $($rest:tt)*) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::RealpathStr($s.to_string()),
        ));
        syscall_test!(@parse_entries $trace; $($rest)*);
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); realpath($s:expr)) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::RealpathStr($s.to_string()),
        ));
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); stat_dir, $($rest:tt)*) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::StatDir,
        ));
        syscall_test!(@parse_entries $trace; $($rest)*);
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); stat_dir) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::StatDir,
        ));
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); stat_file($mode:expr), $($rest:tt)*) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::StatFile($mode),
        ));
        syscall_test!(@parse_entries $trace; $($rest)*);
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); stat_file($mode:expr)) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::StatFile($mode),
        ));
    };
    // Wildcard return
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); _, $($rest:tt)*) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::Int(0),
        ));
        syscall_test!(@parse_entries $trace; $($rest)*);
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); _) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::Int(0),
        ));
    };
    // Void result (for exit-like calls)
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); void, $($rest:tt)*) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::Void,
        ));
        syscall_test!(@parse_entries $trace; $($rest)*);
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); void) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::Void,
        ));
    };
    // Integer result
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); $val:expr, $($rest:tt)*) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::Int($val as i64),
        ));
        syscall_test!(@parse_entries $trace; $($rest)*);
    };
    (@emit_entry $trace:ident; $syscall:ident; ($($args:tt)*); $val:expr) => {
        $trace.push($crate::sys::test_support::t(
            stringify!($syscall),
            syscall_test!(@args $($args)*),
            $crate::sys::test_support::TraceResult::Int($val as i64),
        ));
    };

    // Parse argument list
    (@args) => { vec![] };
    (@args _) => { vec![$crate::sys::test_support::ArgMatcher::Any] };
    (@args _, $($rest:tt)*) => {{
        let mut args = vec![$crate::sys::test_support::ArgMatcher::Any];
        args.extend(syscall_test!(@args $($rest)*));
        args
    }};
    (@args $arg:expr) => { vec![syscall_test!(@one_arg $arg)] };
    (@args $arg:expr, $($rest:tt)*) => {{
        let mut args = vec![syscall_test!(@one_arg $arg)];
        args.extend(syscall_test!(@args $($rest)*));
        args
    }};

    // Single argument conversion
    (@one_arg _) => { $crate::sys::test_support::ArgMatcher::Any };
    (@one_arg $arg:expr) => { $crate::sys::test_support::arg_from($arg) };
}

#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use syscall_test;
