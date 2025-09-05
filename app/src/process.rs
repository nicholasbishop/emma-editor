#![expect(clippy::new_without_default)]

use crate::action::Action;
use crate::buffer::BufferId;
use crate::message::{Message, MessageWriter};
use anyhow::Result;
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::thread::{self, JoinHandle};

pub struct NonInteractiveProcess {
    command: Command,
    child: Option<Child>,
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
            thread_handle: None,
        }
    }

    pub fn run(
        &mut self,
        buf_id: BufferId,
        message_writer: MessageWriter,
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

                message_writer
                    .send(Message::Action(Action::AppendToBuffer(
                        buf_id.clone(),
                        output,
                    )))
                    .unwrap();

                // TODO: add a way to insert text directly.
                // let mut s = buf.text().to_string();
                // s.push_str(&output);
                // buf.set_text(&s);
            }
        });
        self.thread_handle = Some(thread_handle);

        Ok(())
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
