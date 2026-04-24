use std::rc::Rc;

use crate::bstr::ByteWriter;
use crate::builtin;
use crate::expand::word;
use crate::shell::error::{ShellError, VarError};
use crate::shell::state::{FlowSignal, PendingControl, Shell};
use crate::syntax::ast::{Argv0Memo, Command, RedirectionKind, SimpleCommand};
use crate::syntax::word_part::WordPart;
use crate::sys;

use super::and_or::ProcessGroupPlan;
use super::command::execute_command;
use super::pipeline::wait_for_external_child;
#[cfg(test)]
use super::process::ExpandedSimpleCommand;
use super::process::{
    ExpandedRedirection, PreparedProcess, ProcessRedirection, exec_prepared_in_current_process,
    join_boxed_bytes, resolve_command_path, spawn_prepared,
};
use super::redirection::{
    apply_shell_redirection, apply_shell_redirections, default_fd_for_redirection,
};
use super::scratch::ExecScratch;

/// Bare-literal RHS fast path for assignment values.
///
/// Returns `Some(bytes)` when `value` is a single unglobbed
/// `WordPart::Literal` that spans the full raw source (or is an
/// entirely empty word, as in `FOO=`). In those shapes the bytes the
/// assignment should write are literally `value.raw[start..end]`,
/// with no expansion, tilde processing, field splitting, or
/// pathname expansion needed.
///
/// Returning `None` defers to the full `expand_assignment_value`
/// pipeline for anything else — tilde words, glob literals,
/// `$var` / `${var}` / `$(...)` parts, multi-part concatenations,
/// and so on. The discriminator is the parser-recorded
/// `has_glob`, `newlines`, and `parts` layout, so no bytes-level
/// scan runs on the fast path.
#[inline]
fn assignment_literal_fast_path(value: &crate::syntax::ast::Word) -> Option<&[u8]> {
    match &value.parts[..] {
        // Empty RHS: parser emits a word with no parts. The raw
        // source may still be non-empty for quoted-null shapes like
        // `FOO=''` or `FOO=""`, but either way the expansion result
        // is the empty string.
        [] => Some(&[]),
        // `FOO=value` — single unquoted, unglobbed literal that
        // spans the whole raw source.
        [
            WordPart::Literal {
                start: 0,
                end,
                has_glob: false,
                newlines: 0,
                ..
            },
        ] if *end == value.raw.len() => Some(&value.raw),
        // `FOO="quoted value"` — the parser has already materialised
        // the quoted payload; no expansion is needed.
        [WordPart::QuotedLiteral { bytes, newlines: 0 }] => Some(bytes),
        _ => None,
    }
}

pub(super) fn var_error_bytes(e: &VarError) -> Vec<u8> {
    match e {
        VarError::Readonly(name) => ByteWriter::new()
            .bytes(name)
            .bytes(b": readonly variable")
            .finish(),
    }
}

pub(super) struct SavedVar {
    pub(super) name: Box<[u8]>,
    pub(super) value: Option<Vec<u8>>,
    pub(super) was_exported: bool,
}

pub(super) fn save_vars(shell: &Shell, assignments: &[(Vec<u8>, Vec<u8>)]) -> Vec<SavedVar> {
    assignments
        .iter()
        .map(|(name, _)| SavedVar {
            name: name.clone().into(),
            value: shell.get_var(name).map(|s| s.to_vec()),
            was_exported: shell.is_exported(name),
        })
        .collect()
}

/// Slot-cache-aware variant used when we still have the AST
/// [`Assignment`] nodes alongside the expanded name/value pairs.
/// Skips the `ShellMap<Vec<u8>, u32>` name lookup for each
/// assignment by consulting the cached slot index stored in the
/// `Assignment::name_slot` field.
pub(super) fn apply_prefix_assignments_cached(
    shell: &mut Shell,
    ast: &[crate::syntax::ast::Assignment],
    expanded: &[(Vec<u8>, Vec<u8>)],
) -> Result<(), ShellError> {
    debug_assert_eq!(ast.len(), expanded.len());
    for (assignment, (name, value)) in ast.iter().zip(expanded) {
        let slot = assignment.name_slot.resolve(shell.vars_mut(), name);
        shell.set_var_by_slot(slot, name, value).map_err(|e| {
            let msg = var_error_bytes(&e);
            shell.diagnostic(1, &msg)
        })?;
    }
    Ok(())
}

pub(super) fn restore_vars(shell: &mut Shell, saved: Vec<SavedVar>) {
    let mut path_changed = false;
    let mut ifs_changed = false;
    for entry in saved {
        let name: Vec<u8> = entry.name.into();
        if name == b"PATH" {
            path_changed = true;
        }
        if name == b"IFS" {
            ifs_changed = true;
        }
        match entry.value {
            Some(v) => {
                shell.env_set_raw(name.clone(), v);
            }
            None => {
                shell.env_remove_raw(&name);
            }
        }
        if entry.was_exported {
            shell.mark_exported(&name);
        } else {
            shell.unmark_exported(&name);
        }
    }
    if path_changed {
        shell.path_cache_mut().clear();
    }
    if ifs_changed && let Some(s) = shell.expand_scratch.as_mut() {
        s.invalidate_ifs();
    }
}

pub(super) enum BuiltinResult {
    Status(i32),
    UtilityError(i32),
}

fn run_builtin_flow_entry(
    shell: &mut Shell,
    entry: &builtin::BuiltinEntry,
    argv: &[Vec<u8>],
    assignments: &[(Vec<u8>, Vec<u8>)],
) -> Result<BuiltinResult, ShellError> {
    let signal = shell.run_builtin_entry(entry, argv, assignments)?;
    flow_signal_to_result(shell, signal)
}

fn flow_signal_to_result(
    shell: &mut Shell,
    signal: FlowSignal,
) -> Result<BuiltinResult, ShellError> {
    match signal {
        FlowSignal::Continue(status) => Ok(BuiltinResult::Status(status)),
        FlowSignal::UtilityError(status) => Ok(BuiltinResult::UtilityError(status)),
        FlowSignal::Exit(status) => {
            shell.running = false;
            Ok(BuiltinResult::Status(status))
        }
    }
}

/// What `argv[0]` resolved to after consulting the AST memo, the
/// functions map, and the static builtin table. Computed once per
/// `execute_simple` call.
pub(super) enum Argv0Classification {
    Function(Rc<Command>),
    SpecialBuiltin(&'static builtin::BuiltinEntry),
    RegularBuiltin(&'static builtin::BuiltinEntry),
    External,
}

/// True iff `argv[0]` is a single fully-literal word part that expands
/// to exactly its raw bytes. Only literal `argv[0]`s are memoizable -
/// anything involving expansion could change between executions.
fn argv0_is_literal(simple: &SimpleCommand) -> bool {
    let Some(first) = simple.words.first() else {
        return false;
    };
    matches!(
        first.parts.as_slice(),
        [WordPart::Literal {
            start: 0,
            end,
            has_glob: false,
            newlines: 0,
            ..
        }] if *end == first.raw.len(),
    )
}

/// Look up a function body through the per-`SimpleCommand` slot cache,
/// refreshing the cached `Rc<FunctionSlot>` on miss. Returns `None` if
/// no function with that name exists.
fn probe_function_memoized(
    shell: &Shell,
    simple: &SimpleCommand,
    name: &[u8],
) -> Option<Rc<Command>> {
    if let Some(slot) = simple.argv0_slot.borrow().as_ref() {
        if let Some(body) = slot.body.borrow().as_ref().map(Rc::clone) {
            return Some(body);
        }
    }
    match shell.lookup_function_slot(name) {
        Some(slot) => {
            let body = slot.body.borrow().as_ref().map(Rc::clone);
            *simple.argv0_slot.borrow_mut() = Some(slot);
            body
        }
        None => {
            if simple.argv0_slot.borrow().is_some() {
                *simple.argv0_slot.borrow_mut() = None;
            }
            None
        }
    }
}

pub(super) fn classify_argv0(
    shell: &Shell,
    simple: &SimpleCommand,
    argv0: &[u8],
) -> Argv0Classification {
    if matches!(simple.argv0_memo.get(), Argv0Memo::Uncached) {
        let new_memo = if !argv0_is_literal(simple) {
            Argv0Memo::NotLiteral
        } else {
            match builtin::lookup(argv0) {
                Some(entry) if matches!(entry.kind, builtin::BuiltinKind::Special) => {
                    Argv0Memo::LiteralSpecial(entry)
                }
                Some(entry) => Argv0Memo::LiteralRegular(entry),
                None => Argv0Memo::LiteralNoBuiltin,
            }
        };
        simple.argv0_memo.set(new_memo);
    }

    match simple.argv0_memo.get() {
        Argv0Memo::Uncached => unreachable!("memo was just primed"),
        Argv0Memo::NotLiteral => match builtin::lookup(argv0) {
            Some(entry) if matches!(entry.kind, builtin::BuiltinKind::Special) => {
                Argv0Classification::SpecialBuiltin(entry)
            }
            Some(entry) => {
                if let Some(body) = shell.lookup_function(argv0) {
                    Argv0Classification::Function(body)
                } else {
                    Argv0Classification::RegularBuiltin(entry)
                }
            }
            None => {
                if let Some(body) = shell.lookup_function(argv0) {
                    Argv0Classification::Function(body)
                } else {
                    Argv0Classification::External
                }
            }
        },
        Argv0Memo::LiteralSpecial(entry) => Argv0Classification::SpecialBuiltin(entry),
        Argv0Memo::LiteralRegular(entry) => {
            if let Some(body) = probe_function_memoized(shell, simple, argv0) {
                Argv0Classification::Function(body)
            } else {
                Argv0Classification::RegularBuiltin(entry)
            }
        }
        Argv0Memo::LiteralNoBuiltin => {
            if let Some(body) = probe_function_memoized(shell, simple, argv0) {
                Argv0Classification::Function(body)
            } else {
                Argv0Classification::External
            }
        }
    }
}

#[cfg(test)]
pub(super) fn write_xtrace(shell: &mut Shell, expanded: &ExpandedSimpleCommand) {
    write_xtrace_parts(shell, &expanded.assignments, &expanded.argv);
}

fn write_xtrace_parts(shell: &mut Shell, assignments: &[(Vec<u8>, Vec<u8>)], argv: &[Vec<u8>]) {
    if !shell.options.xtrace {
        return;
    }
    // PS4 goes through the full prompt expansion pipeline — including
    // the backslash-escape pass when `bash_prompts` is on — per
    // ps1-prompt-extensions.md § 3.5 / § 9.4. The invisible-region
    // mask is discarded because the xtrace writer is not
    // cursor-positioned.
    let expanded_ps4 = crate::interactive::prompt::expand_ps4(shell);
    // Per spec § 3.5: "When the rendered value of PS4 is longer than
    // a single character, the first character shall be duplicated
    // once per level of subshell nesting, matching bash." We hold the
    // PS4 rendering unchanged at the top level and add one leading
    // copy of its first byte per nested subshell (`++`, `+++`, ...).
    let nesting = shell.subshell_nesting_level as usize;
    let mut line = Vec::with_capacity(expanded_ps4.len() + nesting);
    if expanded_ps4.len() > 1 && nesting > 0 {
        let first = expanded_ps4[0];
        for _ in 0..nesting {
            line.push(first);
        }
    }
    line.extend_from_slice(&expanded_ps4);
    for (name, value) in assignments {
        line.extend_from_slice(name);
        line.push(b'=');
        line.extend_from_slice(value);
        line.push(b' ');
    }
    for (i, word) in argv.iter().enumerate() {
        if i > 0 {
            line.push(b' ');
        }
        line.extend_from_slice(word);
    }
    line.push(b'\n');
    let _ = sys::fd_io::write_all_fd(sys::constants::STDERR_FILENO, &line);
}

pub(super) fn has_command_substitution(simple: &SimpleCommand) -> bool {
    fn word_has_cmd_sub(word: &crate::syntax::ast::Word) -> bool {
        use crate::syntax::word_part::{ExpansionKind, WordPart};
        if !word.parts.is_empty() {
            return word.parts.iter().any(|p| {
                matches!(
                    p,
                    WordPart::Expansion {
                        kind: ExpansionKind::Command { .. },
                        ..
                    }
                )
            });
        }
        let raw: &[u8] = &word.raw;
        raw.windows(2).any(|w| w == b"$(") || raw.contains(&b'`')
    }
    simple
        .assignments
        .iter()
        .any(|a| word_has_cmd_sub(&a.value))
        || simple.words.iter().any(|w| word_has_cmd_sub(w))
}

pub(super) fn execute_simple(
    shell: &mut Shell,
    simple: &SimpleCommand,
    allow_exec_in_place: bool,
) -> Result<i32, ShellError> {
    let mut scratch = shell.take_exec_scratch();
    let result = execute_simple_with_scratch(shell, simple, allow_exec_in_place, &mut scratch);
    shell.recycle_exec_scratch(scratch);
    result
}

fn execute_simple_with_scratch(
    shell: &mut Shell,
    simple: &SimpleCommand,
    allow_exec_in_place: bool,
    scratch: &mut ExecScratch,
) -> Result<i32, ShellError> {
    expand_simple_in_place(shell, simple, scratch)?;

    if let Some(first_word) = simple.words.first() {
        shell.lineno = first_word.line;
    }

    if !scratch.argv.is_empty() || !scratch.assignments.is_empty() {
        write_xtrace_parts(shell, &scratch.assignments, &scratch.argv);
    }

    if scratch.argv.is_empty() {
        let cmd_sub_status = if has_command_substitution(simple) {
            shell.last_status
        } else {
            0
        };
        let guard = match apply_shell_redirections(&scratch.redirections, shell.options.noclobber) {
            Ok(g) => g,
            Err(error) => return Ok(shell.diagnostic_syserr(1, &error).exit_status()),
        };
        debug_assert_eq!(simple.assignments.len(), scratch.assignments.len());
        for (assignment, (name, value)) in simple.assignments.iter().zip(&scratch.assignments) {
            let slot = assignment.name_slot.resolve(shell.vars_mut(), name);
            shell.set_var_by_slot(slot, name, value).map_err(|e| {
                let msg = var_error_bytes(&e);
                shell.diagnostic(1, &msg)
            })?;
        }
        drop(guard);
        return Ok(cmd_sub_status);
    }

    let classification = classify_argv0(shell, simple, &scratch.argv[0]);

    let is_exec_no_cmd = matches!(
        classification,
        Argv0Classification::SpecialBuiltin(entry)
            if entry.name == b"exec" && !scratch.argv.iter().skip(1).any(|a| a == b"--")
    );

    if is_exec_no_cmd {
        let Argv0Classification::SpecialBuiltin(entry) = classification else {
            unreachable!("is_exec_no_cmd implies SpecialBuiltin")
        };
        for redir in &scratch.redirections {
            shell.lineno = redir.line;
            apply_shell_redirection(redir, shell.options.noclobber)
                .map_err(|e| shell.diagnostic_syserr(1, &e))?;
        }
        return match run_builtin_flow_entry(shell, entry, &scratch.argv, &scratch.assignments) {
            Ok(BuiltinResult::Status(status) | BuiltinResult::UtilityError(status)) => Ok(status),
            Err(error) => Err(error),
        };
    }

    if let Argv0Classification::SpecialBuiltin(entry) = classification {
        let _guard = apply_shell_redirections(&scratch.redirections, shell.options.noclobber)
            .map_err(|e| shell.diagnostic_syserr(1, &e))?;
        let result = run_builtin_flow_entry(shell, entry, &scratch.argv, &scratch.assignments);
        drop(_guard);
        return match result {
            Ok(BuiltinResult::UtilityError(status)) if !shell.interactive => {
                Err(ShellError::Status(status))
            }
            Ok(BuiltinResult::Status(status) | BuiltinResult::UtilityError(status)) => Ok(status),
            Err(error) => Err(error),
        };
    }

    if let Argv0Classification::Function(function) = classification {
        let guard = match apply_shell_redirections(&scratch.redirections, shell.options.noclobber) {
            Ok(g) => g,
            Err(error) => return Ok(shell.diagnostic_syserr(1, &error).exit_status()),
        };
        let saved_vars = save_vars(shell, &scratch.assignments);
        if let Err(e) =
            apply_prefix_assignments_cached(shell, &simple.assignments, &scratch.assignments)
        {
            restore_vars(shell, saved_vars);
            drop(guard);
            return Err(e);
        }
        // Move argv out of scratch into shell.positional, preserving
        // the outer Vec capacity. On return we swap it back so the
        // pool reclaims the capacity for the next simple command.
        let mut argv = std::mem::take(&mut scratch.argv);
        argv.remove(0);
        let saved = std::mem::replace(&mut shell.positional, argv);
        shell.function_depth += 1;
        let status = execute_command(shell, &function);
        shell.function_depth = shell.function_depth.saturating_sub(1);
        let used_argv = std::mem::replace(&mut shell.positional, saved);
        scratch.argv = used_argv;
        restore_vars(shell, saved_vars);
        drop(guard);
        return match status {
            Ok(status) => match shell.pending_control {
                Some(PendingControl::Return(return_status)) => {
                    shell.pending_control = None;
                    Ok(return_status)
                }
                _ => Ok(status),
            },
            Err(error) => Err(error),
        };
    }

    if let Argv0Classification::RegularBuiltin(entry) = classification {
        let saved_vars = save_vars(shell, &scratch.assignments);
        let assign_result =
            apply_prefix_assignments_cached(shell, &simple.assignments, &scratch.assignments);
        let result = match assign_result {
            Ok(()) => {
                let r = match apply_shell_redirections(
                    &scratch.redirections,
                    shell.options.noclobber,
                ) {
                    Ok(guard) => {
                        let r = run_builtin_flow_entry(shell, entry, &scratch.argv, &[]);
                        drop(guard);
                        r
                    }
                    Err(e) => Err(shell.diagnostic_syserr(1, &e)),
                };
                restore_vars(shell, saved_vars);
                r
            }
            Err(error) => {
                restore_vars(shell, saved_vars);
                Err(error)
            }
        };
        return match result {
            Ok(BuiltinResult::Status(status) | BuiltinResult::UtilityError(status)) => Ok(status),
            Err(error) => Ok(error.exit_status()),
        };
    }

    // External command. We consume the scratch Vecs (move them into
    // `build_process_from_expanded`); scratch will retain only empty
    // Vecs. This is a cold path relative to builtins and functions,
    // so losing outer-Vec capacity here doesn't materially hurt hot
    // benchmarks.
    {
        for (name, _value) in &scratch.assignments {
            if shell.is_readonly(name) {
                let mut msg = name.clone();
                msg.extend_from_slice(b": readonly variable");
                return Err(shell.diagnostic(1, &msg));
            }
        }
        let argv = std::mem::take(&mut scratch.argv);
        let assignments = std::mem::take(&mut scratch.assignments);
        let redirections = std::mem::take(&mut scratch.redirections);
        let command_name = argv[0].clone();
        let prepared = build_process_from_expanded(shell, argv, assignments, redirections)
            .expect("argv is non-empty");
        if !prepared.path_verified && !prepared.exec_path.contains(&b'/') {
            let _guard = apply_shell_redirections(&prepared.redirections, prepared.noclobber).ok();
            let msg = ByteWriter::new()
                .bytes(&command_name)
                .bytes(b": not found\n")
                .finish();
            let _ = sys::fd_io::write_all_fd(sys::constants::STDERR_FILENO, &msg);
            return Ok(127);
        }
        let desc = join_boxed_bytes(&prepared.argv, b' ');
        if allow_exec_in_place {
            exec_prepared_in_current_process(shell, &prepared, ProcessGroupPlan::None)
        } else if shell.in_subshell {
            let handle = match spawn_prepared(shell, &prepared, ProcessGroupPlan::None) {
                Ok(h) => h,
                Err(error) => return Ok(error.exit_status()),
            };
            let status = wait_for_external_child(shell, &handle, None, Some(&desc))?;
            Ok(status)
        } else {
            let handle = match spawn_prepared(shell, &prepared, ProcessGroupPlan::NewGroup) {
                Ok(h) => h,
                Err(error) => return Ok(error.exit_status()),
            };
            let pgid = handle.pid;
            let _ = sys::tty::set_process_group(pgid, pgid);
            let status = wait_for_external_child(shell, &handle, Some(pgid), Some(&desc))?;
            Ok(status)
        }
    }
}

/// Populate `scratch` with the expansion of `simple`. The scratch's
/// outer `Vec`s are reused (their capacities preserved); any leftover
/// inner `Vec<u8>`s are dropped by `clear()` before repopulation.
pub(super) fn expand_simple_in_place(
    shell: &mut Shell,
    simple: &SimpleCommand,
    scratch: &mut ExecScratch,
) -> Result<(), ShellError> {
    scratch.clear();
    scratch.assignments.reserve(simple.assignments.len());
    for assignment in &simple.assignments {
        let value = if let Some(bytes) = assignment_literal_fast_path(&assignment.value) {
            // Fast path for the pervasive `NAME=literal` shape (e.g.
            // loops like `for i in 1 2 3; do x=$i ...`): skip the full
            // `expand_assignment_value` pipeline (no `with_scratch`,
            // no `expand_parts_into_mode`, no `ExpandOutput` drain)
            // and take the bytes straight from the AST `raw` buffer.
            // A pool-allocated `Vec<u8>` still carries the value so
            // the downstream assignment / env-write paths stay
            // uniform and can recycle the buffer on reset.
            let mut buf = shell.bytes_pool.take();
            buf.extend_from_slice(bytes);
            buf
        } else {
            word::expand_assignment_value(shell, &assignment.value)
                .map_err(|e| shell.expand_to_err(e))?
        };
        scratch.assignments.push((assignment.name.to_vec(), value));
    }

    scratch.argv.reserve(simple.words.len());
    if simple.declaration_context {
        expand_words_declaration_into(shell, &simple.words, &mut scratch.argv)?;
    } else {
        word::expand_words_into(shell, &simple.words, &mut scratch.argv)
            .map_err(|e| shell.expand_to_err(e))?;
    }

    scratch.redirections.reserve(simple.redirections.len());
    for redirection in &simple.redirections {
        let fd = redirection
            .fd
            .unwrap_or_else(|| default_fd_for_redirection(redirection.kind));
        let (target, here_doc_body) = if redirection.kind == RedirectionKind::HereDoc {
            let here_doc = redirection
                .here_doc
                .as_ref()
                .ok_or_else(|| shell.diagnostic(2, b"missing here-document body" as &[u8]))?;
            let body = if here_doc.expand {
                word::expand_here_document(
                    shell,
                    &here_doc.body,
                    &here_doc.body_parts,
                    here_doc.body_line,
                )
                .map_err(|e| shell.expand_to_err(e))?
            } else {
                here_doc.body.to_vec()
            };
            (here_doc.delimiter.to_vec(), Some(body))
        } else {
            let target = word::expand_redirect_word(shell, &redirection.target)
                .map_err(|e| shell.expand_to_err(e))?;
            if matches!(
                redirection.kind,
                RedirectionKind::DupInput | RedirectionKind::DupOutput
            ) && target != b"-"
                && parse_i32_bytes(&target).is_none()
            {
                return Err(shell.diagnostic(
                    1,
                    b"redirection target must be a file descriptor or '-'" as &[u8],
                ));
            }
            (target, None)
        };
        scratch.redirections.push(ExpandedRedirection {
            fd,
            kind: redirection.kind,
            target,
            here_doc_body,
            line: redirection.target.line,
        });
    }

    Ok(())
}

/// Allocating wrapper around [`expand_simple_in_place`] retained for
/// tests and the (now-deprecated) path in `src/exec/redirection.rs`
/// that still returns an owned [`ExpandedSimpleCommand`].
#[cfg(test)]
pub(super) fn expand_simple(
    shell: &mut Shell,
    simple: &SimpleCommand,
) -> Result<ExpandedSimpleCommand, ShellError> {
    let mut scratch = shell.take_exec_scratch();
    let result = expand_simple_in_place(shell, simple, &mut scratch);
    match result {
        Ok(()) => {
            let argv = std::mem::take(&mut scratch.argv);
            let assignments = std::mem::take(&mut scratch.assignments);
            let redirections = std::mem::take(&mut scratch.redirections);
            shell.recycle_exec_scratch(scratch);
            Ok(ExpandedSimpleCommand {
                assignments,
                argv,
                redirections,
            })
        }
        Err(error) => {
            shell.recycle_exec_scratch(scratch);
            Err(error)
        }
    }
}

pub(super) fn parse_i32_bytes(s: &[u8]) -> Option<i32> {
    crate::bstr::parse_i64(s).and_then(|v| i32::try_from(v).ok())
}

pub(super) fn expand_words_declaration_into(
    shell: &mut Shell,
    words: &[crate::syntax::ast::Word],
    result: &mut Vec<Vec<u8>>,
) -> Result<(), ShellError> {
    let mut found_cmd = false;
    for word in words {
        if !found_cmd {
            word::expand_words_into(shell, std::slice::from_ref(word), result)
                .map_err(|e| shell.expand_to_err(e))?;
            if result
                .last()
                .is_some_and(|s: &Vec<u8>| !s.is_empty() && s != b"command")
            {
                found_cmd = true;
            }
        } else if word::word_is_assignment(word) {
            result.push(
                word::expand_word_as_declaration_assignment(shell, word)
                    .map_err(|e| shell.expand_to_err(e))?,
            );
        } else {
            word::expand_words_into(shell, std::slice::from_ref(word), result)
                .map_err(|e| shell.expand_to_err(e))?;
        }
    }
    Ok(())
}

pub(super) fn expand_redirections(
    shell: &mut Shell,
    redirections: &[crate::syntax::ast::Redirection],
) -> Result<Vec<ExpandedRedirection>, ShellError> {
    let mut expanded_vec = Vec::new();
    for redirection in redirections {
        let fd = redirection
            .fd
            .unwrap_or_else(|| default_fd_for_redirection(redirection.kind));
        let (target, here_doc_body) = if redirection.kind == RedirectionKind::HereDoc {
            let here_doc = redirection
                .here_doc
                .as_ref()
                .ok_or_else(|| shell.diagnostic(2, b"missing here-document body" as &[u8]))?;
            let body = if here_doc.expand {
                word::expand_here_document(
                    shell,
                    &here_doc.body,
                    &here_doc.body_parts,
                    here_doc.body_line,
                )
                .map_err(|e| shell.expand_to_err(e))?
            } else {
                here_doc.body.to_vec()
            };
            (here_doc.delimiter.to_vec(), Some(body))
        } else {
            let target = word::expand_redirect_word(shell, &redirection.target)
                .map_err(|e| shell.expand_to_err(e))?;
            if matches!(
                redirection.kind,
                RedirectionKind::DupInput | RedirectionKind::DupOutput
            ) && target != b"-"
                && parse_i32_bytes(&target).is_none()
            {
                return Err(shell.diagnostic(
                    1,
                    b"redirection target must be a file descriptor or '-'" as &[u8],
                ));
            }
            (target, None)
        };
        expanded_vec.push(ExpandedRedirection {
            fd,
            kind: redirection.kind,
            target,
            here_doc_body,
            line: redirection.target.line,
        });
    }
    Ok(expanded_vec)
}

pub(super) fn build_process_from_expanded(
    shell: &Shell,
    argv: Vec<Vec<u8>>,
    assignments: Vec<(Vec<u8>, Vec<u8>)>,
    redirections: Vec<ExpandedRedirection>,
) -> Result<PreparedProcess, ShellError> {
    let program = argv
        .first()
        .ok_or_else(|| shell.diagnostic(1, b"empty command" as &[u8]))?;
    let prefix_path = assignments
        .iter()
        .find(|(name, _)| name == b"PATH")
        .map(|(_, value)| value.as_slice());
    let resolved = resolve_command_path(shell, program, prefix_path);
    let path_verified = resolved.is_some();
    let exec_path: Vec<u8> = resolved.unwrap_or_else(|| program.to_vec());
    let mut child_env = shell.env_for_child();
    child_env.extend(assignments);
    let redirections = redirections
        .into_iter()
        .map(|r| ProcessRedirection {
            fd: r.fd,
            kind: r.kind,
            target: r.target.into_boxed_slice(),
            here_doc_body: r.here_doc_body.map(Vec::into_boxed_slice),
        })
        .collect();
    Ok(PreparedProcess {
        exec_path: exec_path.into(),
        argv: argv
            .into_iter()
            .map(Vec::into_boxed_slice)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        child_env: child_env
            .into_iter()
            .map(|(k, v)| (k.into_boxed_slice(), v.into_boxed_slice()))
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        redirections,
        noclobber: shell.options.noclobber,
        path_verified,
    })
}

#[cfg(test)]
#[allow(unused_imports)]
mod tests {
    use super::*;
    use crate::exec::program::execute_program;
    use crate::exec::test_support::{parse_test, test_shell};
    use crate::shell::state::Shell;
    use crate::syntax::ast::{Assignment, HereDoc, Redirection, Word};
    use crate::sys::test_support::{assert_no_syscalls, run_trace};
    use crate::trace_entries;

    #[test]
    fn save_restore_vars_restores_previous_values() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.env_set_raw(b"FOO".to_vec(), b"original".to_vec());
            shell.mark_exported(b"FOO");

            let assignments = vec![
                (b"FOO".to_vec(), b"temp".to_vec()),
                (b"BAR".to_vec(), b"new".to_vec()),
            ];
            let saved = save_vars(&shell, &assignments);

            shell.set_var(b"FOO", b"temp").unwrap();
            shell.set_var(b"BAR", b"new").unwrap();
            assert_eq!(shell.get_var(b"FOO"), Some(b"temp" as &[u8]));
            assert_eq!(shell.get_var(b"BAR"), Some(b"new" as &[u8]));

            restore_vars(&mut shell, saved);
            assert_eq!(shell.get_var(b"FOO"), Some(b"original" as &[u8]));
            assert!(shell.is_exported(b"FOO"));
            assert_eq!(shell.get_var(b"BAR"), None);
            assert!(!shell.is_exported(b"BAR"));
        });
    }

    #[test]
    fn restore_vars_with_ifs_invalidates_cache() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.env_set_raw(b"IFS".to_vec(), b" ".to_vec());
            // Prime the IFS cache via a first expansion.
            let _ = shell.execute_string(b"x=1").expect("prime");
            assert!(shell.expand_scratch.as_ref().unwrap().ifs_valid);
            let assignments = vec![(b"IFS".to_vec(), b":".to_vec())];
            let saved = save_vars(&shell, &assignments);
            shell.set_var(b"IFS", b":").unwrap();
            assert!(!shell.expand_scratch.as_ref().unwrap().ifs_valid);
            shell.expand_scratch.as_mut().unwrap().ifs_valid = true;
            // The crucial case: restore via the env_mut path must
            // still mark the cache stale so the next expansion re-reads
            // the live IFS.
            restore_vars(&mut shell, saved);
            assert!(!shell.expand_scratch.as_ref().unwrap().ifs_valid);
            assert_eq!(shell.get_var(b"IFS"), Some(b" " as &[u8]));
        });
    }

    #[test]
    fn restore_vars_with_path_clears_cache() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.env_set_raw(b"PATH".to_vec(), b"/usr/bin".to_vec());
            let assignments = vec![(b"PATH".to_vec(), b"/tmp".to_vec())];
            let saved = save_vars(&shell, &assignments);
            shell.set_var(b"PATH", b"/tmp").unwrap();
            restore_vars(&mut shell, saved);
            assert_eq!(shell.get_var(b"PATH"), Some(b"/usr/bin" as &[u8]));
        });
    }

    #[test]
    fn non_special_builtin_prefix_assignments_are_temporary() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.env_set_raw(b"FOO".to_vec(), b"original".to_vec());
            let program = parse_test("FOO=temp true").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"FOO"), Some(b"original" as &[u8]));
        });
    }

    #[test]
    fn special_builtin_prefix_assignments_are_permanent() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let program = parse_test("FOO=permanent :").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"FOO"), Some(b"permanent" as &[u8]));
        });
    }

    #[test]
    fn function_prefix_assignments_are_temporary() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.env_set_raw(b"FOO".to_vec(), b"original".to_vec());
            let program = parse_test("myfn() { :; }; FOO=temp myfn").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"FOO"), Some(b"original" as &[u8]));
        });
    }

    #[test]
    fn non_special_builtin_exit_with_temp_assignments() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let program = parse_test("FOO=bar exit 0").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert!(!shell.running);
        });
    }

    #[test]
    fn assignment_expansion_does_not_field_split() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.env_set_raw(b"IFS".to_vec(), b" ".to_vec());
            shell.env_set_raw(b"X".to_vec(), b"a b c".to_vec());
            let program = parse_test("Y=$X").expect("parse");
            let _status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(shell.get_var(b"Y"), Some(b"a b c" as &[u8]));
        });
    }

    #[test]
    fn xtrace_writes_trace_to_stderr() {
        run_trace(
            trace_entries![write(fd(2), bytes(b"+ echo hello\n")) -> auto],
            || {
                let mut shell = test_shell();
                shell.options.xtrace = true;
                let expanded = ExpandedSimpleCommand {
                    assignments: vec![],
                    argv: vec![b"echo".to_vec(), b"hello".to_vec()],
                    redirections: vec![],
                };
                write_xtrace(&mut shell, &expanded);
            },
        );
    }

    #[test]
    fn xtrace_skipped_when_disabled() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.options.xtrace = false;
            let expanded = ExpandedSimpleCommand {
                assignments: vec![],
                argv: vec![b"echo".to_vec()],
                redirections: vec![],
            };
            write_xtrace(&mut shell, &expanded);
        });
    }

    #[test]
    fn xtrace_includes_assignments() {
        run_trace(
            trace_entries![write(fd(2), bytes(b"+ FOO=bar cmd\n")) -> auto],
            || {
                let mut shell = test_shell();
                shell.options.xtrace = true;
                let expanded = ExpandedSimpleCommand {
                    assignments: vec![(b"FOO".to_vec(), b"bar".to_vec())],
                    argv: vec![b"cmd".to_vec()],
                    redirections: vec![],
                };
                write_xtrace(&mut shell, &expanded);
            },
        );
    }

    #[test]
    fn has_command_substitution_detects_backtick_in_words() {
        assert_no_syscalls(|| {
            let cmd = SimpleCommand {
                words: vec![Word {
                    raw: b"echo `date`".to_vec().into(),
                    parts: Vec::new(),
                    line: 0,
                }],
                ..SimpleCommand::default()
            };
            assert!(has_command_substitution(&cmd));

            let cmd_no_sub = SimpleCommand {
                words: vec![Word {
                    raw: b"plain".to_vec().into(),
                    parts: Vec::new(),
                    line: 0,
                }],
                ..SimpleCommand::default()
            };
            assert!(!has_command_substitution(&cmd_no_sub));
        });
    }

    #[test]
    fn declaration_builtin_expands_assignments_and_words() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let status = shell
                .execute_string(b"command export FOO=bar BAZ")
                .expect("export");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"FOO"), Some(b"bar" as &[u8]));
        });
    }

    #[test]
    fn var_error_bytes_formats_readonly() {
        assert_no_syscalls(|| {
            let err = VarError::Readonly(b"HOME".to_vec().into());
            assert_eq!(var_error_bytes(&err), b"HOME: readonly variable");
        });
    }

    #[test]
    fn parse_i32_bytes_various_inputs() {
        assert_no_syscalls(|| {
            assert_eq!(parse_i32_bytes(b"0"), Some(0));
            assert_eq!(parse_i32_bytes(b"42"), Some(42));
            assert_eq!(parse_i32_bytes(b"-1"), Some(-1));
            assert_eq!(parse_i32_bytes(b"2147483647"), Some(i32::MAX));
            assert_eq!(parse_i32_bytes(b"-2147483648"), Some(i32::MIN));
            assert_eq!(parse_i32_bytes(b"2147483648"), None);
            assert_eq!(parse_i32_bytes(b""), None);
            assert_eq!(parse_i32_bytes(b"abc"), None);
            assert_eq!(parse_i32_bytes(b"-"), None);
        });
    }

    #[test]
    fn has_command_substitution_dollar_paren_in_assignments() {
        assert_no_syscalls(|| {
            let cmd = SimpleCommand {
                assignments: vec![Assignment::new(
                    b"X".to_vec(),
                    Word {
                        raw: b"$(date)".to_vec(),
                        parts: Vec::new(),
                        line: 0,
                    },
                )],
                ..SimpleCommand::default()
            };
            assert!(has_command_substitution(&cmd));

            let cmd_backtick_assign = SimpleCommand {
                assignments: vec![Assignment::new(
                    b"X".to_vec(),
                    Word {
                        raw: b"`date`".to_vec(),
                        parts: Vec::new(),
                        line: 0,
                    },
                )],
                ..SimpleCommand::default()
            };
            assert!(has_command_substitution(&cmd_backtick_assign));

            let cmd_dollar_paren_word = SimpleCommand {
                words: vec![Word {
                    raw: b"echo $(date)".to_vec().into(),
                    parts: Vec::new(),
                    line: 0,
                }],
                ..SimpleCommand::default()
            };
            assert!(has_command_substitution(&cmd_dollar_paren_word));

            let cmd_none = SimpleCommand {
                assignments: vec![Assignment::new(
                    b"X".to_vec(),
                    Word {
                        raw: b"plain".to_vec(),
                        parts: Vec::new(),
                        line: 0,
                    },
                )],
                words: vec![Word {
                    raw: b"echo".to_vec().into(),
                    parts: Vec::new(),
                    line: 0,
                }],
                ..SimpleCommand::default()
            };
            assert!(!has_command_substitution(&cmd_none));
        });
    }

    #[test]
    fn has_command_substitution_via_word_parts() {
        use crate::syntax::word_part::{ExpansionKind, WordPart};
        assert_no_syscalls(|| {
            let cmd_with_parts = SimpleCommand {
                words: vec![Word {
                    raw: b"$(echo hi)".to_vec(),
                    parts: vec![WordPart::Expansion {
                        kind: ExpansionKind::Command {
                            program: std::rc::Rc::new(crate::syntax::ast::Program::default()),
                        },
                        quoted: false,
                    }],
                    line: 0,
                }],
                ..SimpleCommand::default()
            };
            assert!(has_command_substitution(&cmd_with_parts));

            let cmd_no_cmdsub = SimpleCommand {
                words: vec![Word {
                    raw: b"$X".to_vec(),
                    parts: vec![WordPart::Expansion {
                        kind: ExpansionKind::SimpleVar {
                            start: 0,
                            end: 2,
                            cache: crate::shell::vars::CachedVarBinding::default(),
                        },
                        quoted: false,
                    }],
                    line: 0,
                }],
                ..SimpleCommand::default()
            };
            assert!(!has_command_substitution(&cmd_no_cmdsub));
        });
    }

    #[test]
    fn readonly_var_blocks_external_cmd_prefix_assignment() {
        run_trace(
            trace_entries![write(fd(sys::constants::STDERR_FILENO), bytes(b"meiksh: line 1: X: readonly variable\n")) -> auto],
            || {
                let mut shell = test_shell();
                shell.env_set_raw(b"PATH".to_vec(), b"/usr/bin".to_vec());
                shell.mark_readonly(b"X");
                let err = shell
                    .execute_string(b"X=val /nonexistent/cmd")
                    .expect_err("readonly prefix");
                assert_ne!(err.exit_status(), 0);
            },
        );
    }

    #[test]
    fn write_xtrace_with_custom_ps4() {
        run_trace(
            trace_entries![write(fd(2), bytes(b">> echo hi\n")) -> auto],
            || {
                let mut shell = test_shell();
                shell.options.xtrace = true;
                shell.env_set_raw(b"PS4".to_vec(), b">> ".to_vec());
                let expanded = ExpandedSimpleCommand {
                    assignments: vec![],
                    argv: vec![b"echo".to_vec(), b"hi".to_vec()],
                    redirections: vec![],
                };
                write_xtrace(&mut shell, &expanded);
            },
        );
    }

    #[test]
    fn write_xtrace_empty_argv_with_assignments_only() {
        run_trace(
            trace_entries![write(fd(2), bytes(b"+ A=1 B=2 \n")) -> auto],
            || {
                let mut shell = test_shell();
                shell.options.xtrace = true;
                let expanded = ExpandedSimpleCommand {
                    assignments: vec![
                        (b"A".to_vec(), b"1".to_vec()),
                        (b"B".to_vec(), b"2".to_vec()),
                    ],
                    argv: vec![],
                    redirections: vec![],
                };
                write_xtrace(&mut shell, &expanded);
            },
        );
    }

    #[test]
    fn apply_prefix_assignments_readonly_error() {
        run_trace(
            trace_entries![write(fd(sys::constants::STDERR_FILENO), bytes(b"meiksh: RO: readonly variable\n")) -> auto],
            || {
                let mut shell = test_shell();
                shell.mark_readonly(b"RO");
                let ast = vec![crate::syntax::ast::Assignment::new(
                    b"RO".to_vec(),
                    crate::syntax::ast::Word {
                        raw: Vec::new(),
                        parts: Vec::new(),
                        line: 1,
                    },
                )];
                let expanded = vec![(b"RO".to_vec(), b"newval".to_vec())];
                let err = apply_prefix_assignments_cached(&mut shell, &ast, &expanded)
                    .expect_err("readonly should fail");
                assert_ne!(err.exit_status(), 0);
            },
        );
    }

    #[test]
    fn empty_command_redirection_error() {
        run_trace(
            trace_entries![
                fcntl(int(1), _, _) -> int(10),
                open(_, _, _) -> err(sys::constants::EACCES),
                dup2(fd(10), fd(1)) -> fd(1),
                close(fd(10)) -> 0,
                write(fd(2), bytes(b"meiksh: line 1: Permission denied\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                let prog = parse_test("> /forbidden").unwrap();
                let status = execute_program(&mut shell, &prog).unwrap();
                assert_eq!(status, 1);
            },
        );
    }

    #[test]
    fn exec_no_cmd_redirection_error() {
        run_trace(
            trace_entries![
                open(_, _, _) -> err(sys::constants::EACCES),
                write(fd(2), bytes(b"meiksh: line 1: Permission denied\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                let prog = parse_test("exec > /forbidden").unwrap();
                let err = execute_program(&mut shell, &prog).unwrap_err();
                assert_eq!(err.exit_status(), 1);
            },
        );
    }

    #[test]
    fn exec_no_cmd_assignment_error() {
        run_trace(
            trace_entries![
                write(fd(2), bytes(b"meiksh: line 1: x: readonly variable\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell.mark_readonly(b"x");
                let prog = parse_test("x=2 exec").unwrap();
                let err = execute_program(&mut shell, &prog).unwrap_err();
                assert_eq!(err.exit_status(), 1);
            },
        );
    }

    #[test]
    fn function_redirection_error() {
        run_trace(
            trace_entries![
                fcntl(int(1), _, _) -> int(10),
                open(_, _, _) -> err(sys::constants::EACCES),
                dup2(fd(10), fd(1)) -> fd(1),
                close(fd(10)) -> 0,
                write(fd(2), bytes(b"meiksh: line 1: Permission denied\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                let fn_prog = parse_test("myfn() { true; }").unwrap();
                execute_program(&mut shell, &fn_prog).unwrap();
                let prog = parse_test("myfn > /forbidden").unwrap();
                let status = execute_program(&mut shell, &prog).unwrap();
                assert_eq!(status, 1);
            },
        );
    }

    #[test]
    fn builtin_prefix_assignment_error() {
        run_trace(
            trace_entries![
                write(fd(2), bytes(b"meiksh: line 1: x: readonly variable\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell.mark_readonly(b"x");
                let prog = parse_test("x=2 echo hi").unwrap();
                let status = execute_program(&mut shell, &prog).unwrap();
                assert_eq!(status, 1);
            },
        );
    }

    #[test]
    fn external_command_prefix_assignment_error() {
        run_trace(
            trace_entries![
                write(fd(2), bytes(b"meiksh: line 1: x: readonly variable\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell.mark_readonly(b"x");
                let prog = parse_test("x=2 /bin/true").unwrap();
                let err = execute_program(&mut shell, &prog).unwrap_err();
                assert_eq!(err.exit_status(), 1);
            },
        );
    }

    #[test]
    fn external_command_spawn_error_subshell() {
        run_trace(
            trace_entries![
                fork() -> pid(123), child: [
                    fork() -> err(sys::constants::ENOMEM),
                    write(fd(2), bytes(b"/bin/fail: Cannot allocate memory\n")) -> auto,

                ],
                waitpid(123, _) -> status(1),
            ],
            || {
                let mut shell = test_shell();
                let prog = parse_test("( /bin/fail )").unwrap();
                let status = execute_program(&mut shell, &prog).unwrap();
                assert_eq!(status, 1);
            },
        );
    }

    #[test]
    fn external_command_spawn_error_main() {
        run_trace(
            trace_entries![
                fork() -> err(sys::constants::ENOMEM),
                write(fd(2), bytes(b"/bin/fail: Cannot allocate memory\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                let prog = parse_test("/bin/fail").unwrap();
                let status = execute_program(&mut shell, &prog).unwrap();
                assert_eq!(status, 1);
            },
        );
    }

    #[test]
    fn assignment_only_command_without_words() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let program = parse_test("X=1 Y=2").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"X"), Some(b"1" as &[u8]));
            assert_eq!(shell.get_var(b"Y"), Some(b"2" as &[u8]));
        });
    }

    #[test]
    fn test_execute_expanded_readonly() {
        run_trace(
            trace_entries![
                write(fd(2), bytes(b"meiksh: line 1: RO: readonly variable\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell.mark_readonly(b"RO");
                let prog = parse_test("RO=val /bin/echo").unwrap();
                let err = execute_program(&mut shell, &prog).unwrap_err();
                assert_eq!(err.exit_status(), 1);
            },
        );
    }
    #[test]
    fn prefix_assignment_readonly_external_error() {
        run_trace(
            trace_entries![
                write(fd(2), bytes(b"meiksh: line 1: y: readonly variable\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell.mark_readonly(b"y");
                // Put y=2 in a position where the assignment loop evaluates the second item, hitting the end of the block. Wait, the loop condition returns early. To hit the loop increment/continue, we just need a successful assignment followed by a failing one, or just a successful one.
                let prog = parse_test("x=1 y=2 /bin/true").unwrap();
                let err = execute_program(&mut shell, &prog).unwrap_err();
                assert_eq!(err.exit_status(), 1);
            },
        );
    }

    #[test]
    fn argv0_is_literal_returns_false_for_empty_words() {
        assert_no_syscalls(|| {
            let cmd = SimpleCommand::default();
            assert!(!argv0_is_literal(&cmd));
        });
    }

    #[test]
    fn function_call_reuses_cached_slot_on_second_invocation() {
        // First call primes the argv0_slot; second call goes through the
        // early-return body-cache branch in probe_function_memoized.
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let prog = parse_test("f() { true; }; f; f").unwrap();
            let status = execute_program(&mut shell, &prog).unwrap();
            assert_eq!(status, 0);
        });
    }

    #[test]
    fn classify_argv0_not_literal_with_no_match_falls_back_to_external() {
        // Drives the NotLiteral arm of `classify_argv0` with a name
        // that resolves to neither a builtin nor a function so the
        // External branch (line 270) is taken.
        assert_no_syscalls(|| {
            let shell = test_shell();
            let simple = SimpleCommand::default();
            simple.argv0_memo.set(Argv0Memo::NotLiteral);
            let result = classify_argv0(&shell, &simple, b"definitely_not_a_command_xyz");
            assert!(matches!(result, Argv0Classification::External));
        });
    }

    #[test]
    fn probe_function_memoized_returns_cached_body_directly() {
        // Pre-populate the per-SimpleCommand argv0_slot with a slot
        // that has a body so that `probe_function_memoized` short-
        // circuits via the cached early-return arm at line 215.
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let _ = shell.execute_string(b"f() { :; }");
            let simple = SimpleCommand::default();
            let slot = shell.lookup_function_slot(b"f").expect("slot");
            *simple.argv0_slot.borrow_mut() = Some(slot);
            let result = probe_function_memoized(&shell, &simple, b"f");
            assert!(result.is_some());
        });
    }

    #[test]
    fn function_slot_cleared_after_function_removed() {
        // Probe with a stale, non-None `argv0_slot` after the function
        // has been removed — exercises line 226's slot clear.
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let _ = shell.execute_string(b"f() { :; }");
            let simple = SimpleCommand::default();
            let slot = shell.lookup_function_slot(b"f").expect("slot");
            *simple.argv0_slot.borrow_mut() = Some(slot);
            shell.unset_function(b"f");
            let result = probe_function_memoized(&shell, &simple, b"f");
            assert!(result.is_none());
            assert!(simple.argv0_slot.borrow().is_none());
        });
    }

    #[test]
    fn non_literal_argv0_classifies_as_special_builtin() {
        // Dispatch a special builtin through a parameter expansion so
        // argv0 is not a literal (NotLiteral memo → SpecialBuiltin arm).
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let prog = parse_test("a=:; $a").unwrap();
            let status = execute_program(&mut shell, &prog).unwrap();
            assert_eq!(status, 0);
        });
    }

    #[test]
    fn non_literal_argv0_classifies_as_function_over_regular_builtin() {
        // Shadow a regular builtin with a function and dispatch via
        // expansion; NotLiteral arm prefers the function body.
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let prog = parse_test("true() { :; }; a=true; $a").unwrap();
            let status = execute_program(&mut shell, &prog).unwrap();
            assert_eq!(status, 0);
        });
    }

    #[test]
    fn non_literal_argv0_classifies_as_function_without_builtin() {
        // A plain function invoked through an expanded argv0 hits the
        // NotLiteral → no-builtin → Function branch.
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let prog = parse_test("myfn() { :; }; a=myfn; $a").unwrap();
            let status = execute_program(&mut shell, &prog).unwrap();
            assert_eq!(status, 0);
        });
    }

    #[test]
    fn literal_regular_builtin_shadowed_by_function_dispatches_function() {
        // Direct literal argv0 that matches a regular builtin which is
        // also shadowed by a user function (LiteralRegular → Function).
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let prog = parse_test("true() { :; }; true").unwrap();
            let status = execute_program(&mut shell, &prog).unwrap();
            assert_eq!(status, 0);
        });
    }

    #[test]
    fn function_prefix_assignment_readonly_restores_state() {
        // Readonly prefix assignment on a function call hits the
        // apply_prefix_assignments_cached error path that restores saved
        // variables and closes the redirection guard.
        run_trace(
            trace_entries![
                write(fd(2), bytes(b"meiksh: line 1: RO: readonly variable\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell.mark_readonly(b"RO");
                let prog = parse_test("myfn() { :; }; RO=v myfn").unwrap();
                let err = execute_program(&mut shell, &prog).unwrap_err();
                assert_eq!(err.exit_status(), 1);
            },
        );
    }
}
