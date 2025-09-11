use crate::action::Action;
use crate::buffer::BufferId;
use crate::command_line::CommandLine;
use crate::message::{Message, MessageWriter};
use anyhow::Result;
use std::io::Read;
use std::process::Stdio;
use std::thread::{self, JoinHandle};

pub struct NonInteractiveProcess {
    command_line: CommandLine,
    is_running: bool,
    thread_handle: Option<JoinHandle<()>>,
}

impl NonInteractiveProcess {
    pub fn new() -> Self {
        Self {
            command_line: CommandLine::default(),
            is_running: false,
            thread_handle: None,
        }
    }

    pub fn run(
        &mut self,
        command_line: CommandLine,
        buf_id: BufferId,
        message_writer: MessageWriter,
    ) -> Result<()> {
        // TODO
        assert!(!self.is_running);

        self.command_line = command_line;

        self.is_running = true;
        let mut child = self
            .command_line
            .to_command()
            .stdout(Stdio::piped())
            .spawn()?;
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

                    message_writer
                        .send(Message::Action(Action::ProcessFinished(
                            buf_id.clone(),
                        )))
                        .unwrap();

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

    pub fn rerun(
        &mut self,
        buf_id: BufferId,
        message_writer: MessageWriter,
    ) -> Result<()> {
        self.run(self.command_line.clone(), buf_id, message_writer)
    }

    pub fn set_finished(&mut self) {
        assert!(self.is_running);

        self.thread_handle.take().unwrap().join().unwrap();
        self.is_running = false;
    }
}
