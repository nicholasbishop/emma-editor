#![expect(clippy::new_without_default)]

use crate::buffer::BufferId;
use crate::message::ToGtkMsg;
use anyhow::{Context, Result};
use std::io::{PipeWriter, Read, Write};
use std::os::fd::{AsFd, BorrowedFd};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::thread::{self, JoinHandle};

pub struct NonInteractiveProcess {
    command: Command,
    child: Option<Child>,
    // TODO: pipe with both stdout and stderr
    output: Option<ChildStdout>,
    thread_handle: Option<JoinHandle<()>>,
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
            thread_handle: None,
        }
    }

    pub fn run(
        &mut self,
        buf_id: BufferId,
        mut to_gtk_writer: PipeWriter,
    ) -> Result<()> {
        // TODO
        assert!(self.child.is_none());

        let mut child = self.command.stdout(Stdio::piped()).spawn()?;
        let mut output = Some(child.stdout.take().unwrap());

        // TODO: unwraps
        let thread_handle = thread::spawn(move || {
            // Read from the FD until we can't (with some
            // kind of stopping point, in case the FD keeps
            // returning a flood of data?)
            loop {
                let output = {
                    let output = output.as_mut().unwrap();
                    let mut buf = vec![0; 1024];
                    let len = output.read(&mut buf).unwrap();
                    buf.truncate(len);
                    buf
                };

                if output.is_empty() {
                    // Process finished.
                    let _status = child.wait().unwrap();
                    return;
                }

                // TODO: not great
                let output = String::from_utf8(output).unwrap();

                serde_json::to_writer(
                    to_gtk_writer.try_clone().unwrap(),
                    &ToGtkMsg::AppendToBuffer(buf_id.clone(), output),
                )
                .unwrap();
                to_gtk_writer.write_all(b"\n").unwrap();

                // TODO: add a way to insert text directly.
                // let mut s = buf.text().to_string();
                // s.push_str(&output);
                // buf.set_text(&s);
            }
        });
        self.thread_handle = Some(thread_handle);

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
