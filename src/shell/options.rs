use crate::bstr::ByteWriter;

#[derive(Clone, Debug, Default)]
pub struct ShellOptions {
    pub allexport: bool,
    pub command_string: Option<Box<[u8]>>,
    pub errexit: bool,
    pub syntax_check_only: bool,
    pub force_interactive: bool,
    pub hashall: bool,
    pub monitor: bool,
    pub noclobber: bool,
    pub noglob: bool,
    pub notify: bool,
    pub nounset: bool,
    pub pipefail: bool,
    pub verbose: bool,
    pub xtrace: bool,
    pub script_path: Option<Vec<u8>>,
    pub shell_name_override: Option<Box<[u8]>>,
    pub positional: Vec<Vec<u8>>,
    pub vi_mode: bool,
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
    pub fn set_short_option(&mut self, ch: u8, enabled: bool) -> Result<(), OptionError> {
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

    pub fn set_named_option(&mut self, name: &[u8], enabled: bool) -> Result<(), OptionError> {
        if name == b"pipefail" {
            self.pipefail = enabled;
            return Ok(());
        }
        if name == b"vi" {
            self.vi_mode = enabled;
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

    pub fn reportable_options(&self) -> [(&'static [u8], bool); 12] {
        [
            (b"allexport" as &[u8], self.allexport),
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
            (b"xtrace", self.xtrace),
        ]
    }
}

#[derive(Debug)]
pub enum OptionError {
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
    use super::ShellOptions;

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
