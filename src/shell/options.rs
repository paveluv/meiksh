use crate::bstr::ByteWriter;

#[derive(Clone, Debug, Default)]
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
        let Some((_, letter)) = REPORTABLE_OPTION_NAMES
            .iter()
            .find(|(option_name, _)| *option_name == name)
        else {
            return Err(OptionError::InvalidName(name.into()));
        };
        self.set_short_option(*letter, enabled)
    }

    pub(crate) fn reportable_options(&self) -> [(&'static [u8], bool); 14] {
        [
            (b"allexport" as &[u8], self.allexport),
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
        let mut opts = ShellOptions::default();
        opts.set_named_option(b"emacs", true).expect("emacs on");
        assert!(opts.emacs_mode);
        opts.set_named_option(b"vi", true).expect("vi on");
        assert!(opts.vi_mode);
        assert!(!opts.emacs_mode);
    }

    #[test]
    fn set_named_option_off_leaves_other_mode_alone() {
        let mut opts = ShellOptions::default();
        opts.set_named_option(b"emacs", true).expect("emacs on");
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
