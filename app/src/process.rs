#![expect(clippy::new_without_default)]

use anyhow::{Context, Result};
use std::io::Read;
use std::os::fd::{AsFd, BorrowedFd};
use std::process::{Child, ChildStdout, Command, Stdio};

pub struct NonInteractiveProcess {
    command: Command,
    child: Option<Child>,
    // TODO: pipe with both stdout and stderr
    output: Option<ChildStdout>,
}

impl NonInteractiveProcess {
    pub fn new() -> Self {
        // TODO: just for initial testing
        let mut command = Command::new("echo");
        command.arg("hello!");

        Self {
            command,
            child: None,
            output: None,
        }
    }

    pub fn run(&mut self) -> Result<()> {
        // TODO
        assert!(self.child.is_none());

        let mut child = self.command.stdout(Stdio::piped()).spawn()?;
        self.output = Some(child.stdout.take().unwrap());
        self.child = Some(child);

        Ok(())
    }

    pub fn output_fd(&self) -> BorrowedFd<'_> {
        self.output.as_ref().unwrap().as_fd()
    }

    pub fn read_output(&mut self) -> Result<Vec<u8>> {
        let output = self.output.as_mut().context("not running")?;
        let mut buf = vec![0; 1024];
        let len = output.read(&mut buf)?;
        buf.truncate(len);
        Ok(buf)
    }

    // TODO
    #[allow(unused)]
    pub fn is_running(&self) -> bool {
        self.child.is_some()
    }

    pub fn wait(&mut self) {
        let mut child = self.child.take().unwrap();
        let _status = child.wait().unwrap();
        // TODO: do something with errors.
    }
}
