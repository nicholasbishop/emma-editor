use anyhow::Result;
use nix::pty::PtyMaster;

pub struct Shell {
    master: PtyMaster,
}
impl Shell {
    pub fn new() -> Result<Shell> {
        todo!();
    }
}
