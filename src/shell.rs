use anyhow::Result;
use nix::pty::PtyMaster;

pub struct Shell {
    _master: PtyMaster,
}

impl Shell {
    pub fn _new() -> Result<Shell> {
        todo!();
    }
}
