use crate::bstr::ByteWriter;

/// Exclusive prompt-language selector. At most one language may be
/// active at any time; `Posix` means no non-POSIX prompt escapes are
/// decoded. Future values (for example `Zsh`, `Ksh`) slot in here and
/// inherit the mutual exclusion rule for free.
///
/// The slot is currently flipped by `set -o bash_prompts` (the
/// bash-style prompt-escape language, named after its source shell per
/// the zsh convention of provenance-prefixed option names; see
/// `docs/features/ps1-prompt-extensions.md` § 2.2). Today the slot and
/// that option are effectively synonyms because `bash_prompts` is the
/// only non-POSIX prompt feature gated this way. The enum is named
/// `PromptsMode` (matching the `bash_prompts` option) rather than a
/// broader "compat mode" because its scope is strictly prompt
/// expansion: non-prompt bash-isms we borrow later will get their own
/// independent `bash_*` options rather than riding on this selector.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum PromptsMode {
    /// Strict POSIX. This is the meiksh default: a fresh interactive or
    /// non-interactive shell starts in this mode.
    #[default]
    Posix,
    /// Bash-style prompt expansion (`\u`, `\h`, `\w`, `\D{...}`, `\[`,
    /// `\]`, `\!`, and so on). Enabled via `set -o bash_prompts`.
    Bash,
}

#[derive(Clone, Debug)]
pub(crate) struct ShellOptions {
    pub(crate) allexport: bool,
    pub(crate) command_string: Option<Box<[u8]>>,
    pub(crate) errexit: bool,
    pub(crate) syntax_check_only: bool,
    pub(crate) force_interactive: bool,
    pub(crate) hashall: bool,
    pub(crate) monitor: bool,
    pub(crate) noclobber: bool,
    pub(crate) noglob: bool,
    pub(crate) notify: bool,
    pub(crate) nounset: bool,
    pub(crate) pipefail: bool,
    pub(crate) verbose: bool,
    pub(crate) xtrace: bool,
    pub(crate) script_path: Option<Vec<u8>>,
    pub(crate) shell_name_override: Option<Box<[u8]>>,
    pub(crate) positional: Vec<Vec<u8>>,
    pub(crate) vi_mode: bool,
    pub(crate) emacs_mode: bool,
    /// Current prompt-language selection (default `Posix`). `set -o
    /// bash_prompts` / `set +o bash_prompts` move this between `Posix`
    /// and `Bash`.
    pub(crate) prompts_mode: PromptsMode,
}

impl Default for ShellOptions {
    /// Meiksh defaults match bash: `emacs` editing mode is **on**, all
    /// other toggleable options are off, and the prompts-mode slot sits
    /// in the strict-POSIX position. A fresh interactive shell
    /// therefore enters its REPL with emacs-style line editing without
    /// the user having to run `set -o emacs` or ship a `.profile`.
    /// Non-interactive shells never enter the REPL so the flag has no
    /// observable effect on scripts, but `set -o` still reports
    /// `emacs on` there (mirroring bash).
    ///
    /// See `docs/features/emacs-editing-mode.md` § 2.5 for the
    /// normative statement of this default.
    fn default() -> Self {
        Self {
            allexport: false,
            command_string: None,
            errexit: false,
            syntax_check_only: false,
            force_interactive: false,
            hashall: false,
            monitor: false,
            noclobber: false,
            noglob: false,
            notify: false,
            nounset: false,
            pipefail: false,
            verbose: false,
            xtrace: false,
            script_path: None,
            shell_name_override: None,
            positional: Vec::new(),
            vi_mode: false,
            emacs_mode: true,
            prompts_mode: PromptsMode::Posix,
        }
    }
}

const REPORTABLE_OPTION_NAMES: [(&[u8], u8); 11] = [
    (b"allexport", b'a'),
    (b"errexit", b'e'),
    (b"hashall", b'h'),
    (b"monitor", b'm'),
    (b"noclobber", b'C'),
    (b"noglob", b'f'),
    (b"noexec", b'n'),
    (b"notify", b'b'),
    (b"nounset", b'u'),
    (b"verbose", b'v'),
    (b"xtrace", b'x'),
];

impl ShellOptions {
    pub(crate) fn set_short_option(&mut self, ch: u8, enabled: bool) -> Result<(), OptionError> {
        match ch {
            b'a' => self.allexport = enabled,
            b'b' => self.notify = enabled,
            b'C' => self.noclobber = enabled,
            b'e' => self.errexit = enabled,
            b'f' => self.noglob = enabled,
            b'h' => self.hashall = enabled,
            b'i' => self.force_interactive = enabled,
            b'm' => self.monitor = enabled,
            b'n' => self.syntax_check_only = enabled,
            b'u' => self.nounset = enabled,
            b'v' => self.verbose = enabled,
            b'x' => self.xtrace = enabled,
            _ => return Err(OptionError::InvalidShort(ch)),
        }
        Ok(())
    }

    pub(crate) fn set_named_option(
        &mut self,
        name: &[u8],
        enabled: bool,
    ) -> Result<(), OptionError> {
        if name == b"pipefail" {
            self.pipefail = enabled;
            return Ok(());
        }
        if name == b"vi" {
            self.vi_mode = enabled;
            // The emacs and vi editing modes are mutually exclusive,
            // per emacs-editing-mode.md § 2.2. Flipping one on flips
            // the other off; flipping either off leaves the other
            // alone.
            if enabled {
                self.emacs_mode = false;
            }
            return Ok(());
        }
        if name == b"emacs" {
            self.emacs_mode = enabled;
            if enabled {
                self.vi_mode = false;
            }
            return Ok(());
        }
        if name == b"bash_prompts" {
            // The prompts-mode slot is a single-valued selector.
            // Enabling `bash_prompts` sets it to `Bash`; disabling it
            // returns the selector to `Posix`. Per
            // ps1-prompt-extensions.md § 2.2, disabling the
            // currently-active prompts option does NOT reactivate a
            // previously-selected sibling.
            self.prompts_mode = if enabled {
                PromptsMode::Bash
            } else {
                PromptsMode::Posix
            };
            return Ok(());
        }
        let Some((_, letter)) = REPORTABLE_OPTION_NAMES
            .iter()
            .find(|(option_name, _)| *option_name == name)
        else {
            return Err(OptionError::InvalidName(name.into()));
        };
        self.set_short_option(*letter, enabled)
    }

    pub(crate) fn reportable_options(&self) -> [(&'static [u8], bool); 15] {
        [
            (b"allexport" as &[u8], self.allexport),
            (
                b"bash_prompts",
                matches!(self.prompts_mode, PromptsMode::Bash),
            ),
            (b"emacs", self.emacs_mode),
            (b"errexit", self.errexit),
            (b"hashall", self.hashall),
            (b"monitor", self.monitor),
            (b"noclobber", self.noclobber),
            (b"noglob", self.noglob),
            (b"noexec", self.syntax_check_only),
            (b"notify", self.notify),
            (b"nounset", self.nounset),
            (b"pipefail", self.pipefail),
            (b"verbose", self.verbose),
            (b"vi", self.vi_mode),
            (b"xtrace", self.xtrace),
        ]
    }
}

#[derive(Debug)]
pub(crate) enum OptionError {
    InvalidShort(u8),
    InvalidName(Box<[u8]>),
}

pub(super) fn option_error_message(e: &OptionError) -> Vec<u8> {
    match e {
        OptionError::InvalidShort(ch) => ByteWriter::new()
            .bytes(b"invalid option: ")
            .byte(*ch)
            .finish(),
        OptionError::InvalidName(name) => ByteWriter::new()
            .bytes(b"invalid option name: ")
            .bytes(name)
            .finish(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_short_option_accepts_new_options() {
        let mut opts = ShellOptions::default();
        opts.set_short_option(b'e', true).expect("set -e");
        assert!(opts.errexit);
        opts.set_short_option(b'e', false).expect("set +e");
        assert!(!opts.errexit);

        opts.set_short_option(b'x', true).expect("set -x");
        assert!(opts.xtrace);
        opts.set_short_option(b'x', false).expect("set +x");
        assert!(!opts.xtrace);

        opts.set_short_option(b'b', true).expect("set -b");
        assert!(opts.notify);

        opts.set_short_option(b'h', true).expect("set -h");
        assert!(opts.hashall);

        opts.set_short_option(b'm', true).expect("set -m");
    }

    #[test]
    fn set_named_option_accepts_new_options() {
        let mut opts = ShellOptions::default();
        opts.set_named_option(b"errexit", true).expect("errexit");
        assert!(opts.errexit);
        opts.set_named_option(b"xtrace", true).expect("xtrace");
        assert!(opts.xtrace);
        opts.set_named_option(b"notify", true).expect("notify");
        assert!(opts.notify);
        opts.set_named_option(b"hashall", true).expect("hashall");
        assert!(opts.hashall);
        opts.set_named_option(b"monitor", true).expect("monitor");
        opts.set_named_option(b"vi", true).expect("vi");
        assert!(opts.vi_mode);
        opts.set_named_option(b"vi", false).expect("vi off");
        assert!(!opts.vi_mode);
    }

    #[test]
    fn default_options_enable_emacs_mode() {
        // `docs/features/emacs-editing-mode.md` § 2.5 specifies that a
        // fresh shell starts in emacs mode, matching bash.
        let opts = ShellOptions::default();
        assert!(opts.emacs_mode);
        assert!(!opts.vi_mode);
    }

    #[test]
    fn set_named_option_emacs_flips_vi() {
        let mut opts = ShellOptions::default();
        opts.set_named_option(b"vi", true).expect("vi on");
        assert!(opts.vi_mode);
        assert!(!opts.emacs_mode);
        opts.set_named_option(b"emacs", true).expect("emacs on");
        assert!(opts.emacs_mode);
        assert!(!opts.vi_mode);
    }

    #[test]
    fn set_named_option_vi_flips_emacs() {
        // Default is already emacs=on; setting vi flips it off.
        let mut opts = ShellOptions::default();
        assert!(opts.emacs_mode);
        opts.set_named_option(b"vi", true).expect("vi on");
        assert!(opts.vi_mode);
        assert!(!opts.emacs_mode);
    }

    #[test]
    fn set_named_option_off_leaves_other_mode_alone() {
        let mut opts = ShellOptions::default();
        opts.set_named_option(b"emacs", false).expect("emacs off");
        assert!(!opts.emacs_mode);
        assert!(!opts.vi_mode);
    }

    #[test]
    fn reportable_options_lists_both_editing_modes() {
        let mut opts = ShellOptions::default();
        opts.emacs_mode = true;
        let reported = opts.reportable_options();
        let names: Vec<&[u8]> = reported.iter().map(|(n, _)| *n).collect();
        assert!(names.contains(&b"emacs".as_slice()));
        assert!(names.contains(&b"vi".as_slice()));
        let emacs = reported.iter().find(|(n, _)| *n == b"emacs").unwrap();
        assert!(emacs.1);
        let vi = reported.iter().find(|(n, _)| *n == b"vi").unwrap();
        assert!(!vi.1);
    }

    #[test]
    fn bash_prompts_named_option_toggles_selector() {
        let mut opts = ShellOptions::default();
        assert_eq!(opts.prompts_mode, PromptsMode::Posix);
        opts.set_named_option(b"bash_prompts", true)
            .expect("bash_prompts on");
        assert_eq!(opts.prompts_mode, PromptsMode::Bash);
        opts.set_named_option(b"bash_prompts", false)
            .expect("bash_prompts off");
        assert_eq!(opts.prompts_mode, PromptsMode::Posix);
    }

    #[test]
    fn bash_prompts_shows_up_in_reportable_options() {
        let mut opts = ShellOptions::default();
        let reported = opts.reportable_options();
        let row = reported
            .iter()
            .find(|(n, _)| *n == b"bash_prompts")
            .expect("bash_prompts row");
        assert!(!row.1, "default bash_prompts should be off");

        opts.set_named_option(b"bash_prompts", true).unwrap();
        let reported = opts.reportable_options();
        let row = reported
            .iter()
            .find(|(n, _)| *n == b"bash_prompts")
            .expect("bash_prompts row");
        assert!(row.1, "reported bash_prompts should flip on");
    }

    #[test]
    fn old_bash_compat_option_name_is_rejected() {
        // The option was renamed from `bash_compat` to `bash_prompts`
        // at 0.1.0; the old name is not an alias. See
        // `docs/features/ps1-prompt-extensions.md` § 2.1.
        let mut opts = ShellOptions::default();
        let err = opts.set_named_option(b"bash_compat", true);
        assert!(
            matches!(err, Err(OptionError::InvalidName(_))),
            "legacy `bash_compat` must surface an invalid-name error, got {err:?}"
        );
    }

    #[test]
    fn reportable_options_includes_new_options() {
        let mut opts = ShellOptions::default();
        opts.errexit = true;
        opts.xtrace = true;
        let reported = opts.reportable_options();
        let names: Vec<&[u8]> = reported.iter().map(|(n, _)| *n).collect();
        assert!(names.contains(&b"errexit".as_slice()));
        assert!(names.contains(&b"xtrace".as_slice()));
        assert!(names.contains(&b"notify".as_slice()));
        assert!(names.contains(&b"hashall".as_slice()));
        assert!(names.contains(&b"monitor".as_slice()));
        let errexit = reported.iter().find(|(n, _)| *n == b"errexit").unwrap();
        assert!(errexit.1);
        let xtrace = reported.iter().find(|(n, _)| *n == b"xtrace").unwrap();
        assert!(xtrace.1);
    }
}
