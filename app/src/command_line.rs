use std::ffi::OsString;
use std::process::Command;

// The `std::process::Command` type is very awkward to work with (it
// doesn't even support `Clone` for example), so make a more Rust-y
// wrapper.
#[derive(Clone, Default)]
pub struct CommandLine {
    program: OsString,
    args: Vec<OsString>,
}

impl CommandLine {
    pub fn to_command(&self) -> Command {
        let mut cmd = Command::new(&self.program);
        cmd.args(&self.args);
        cmd
    }

    pub fn from_string(s: &str) -> Self {
        // TODO: unwrap
        let parts = shlex::split(s).unwrap();
        let program = parts.get(0).map(OsString::from).unwrap_or_default();
        Self {
            program,
            args: parts.iter().skip(1).map(OsString::from).collect(),
        }
    }
}
