use crate::buffer::BufferId;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, BufReader, PipeReader, PipeWriter, Write};
use std::os::fd::{AsRawFd, RawFd};

#[derive(Debug, Clone, Eq, PartialEq, Deserialize, Serialize)]
pub enum Message {
    /// Exit the whole application.
    Close,

    // TODO: not sure if we want something more specific. This is used
    // for appending subprocess output to a buffer.
    AppendToBuffer(BufferId, String),
}

pub struct MessageReader(BufReader<PipeReader>);

impl MessageReader {
    pub fn as_raw_fd(&self) -> RawFd {
        self.0.get_ref().as_raw_fd()
    }

    pub fn read(&mut self) -> Result<Message> {
        let mut msg = Vec::new();
        self.0.read_until(b'\n', &mut msg)?;
        Ok(serde_json::from_slice(&msg)?)
    }
}

pub struct MessageWriter(PipeWriter);

impl MessageWriter {
    pub fn send(&self, msg: Message) -> Result<()> {
        serde_json::to_writer(&self.0, &msg)?;
        (&self.0).write_all(b"\n")?;
        Ok(())
    }

    pub fn try_clone(&self) -> Result<Self> {
        Ok(Self(self.0.try_clone()?))
    }
}

pub fn create_message_pipe() -> Result<(MessageReader, MessageWriter)> {
    let (reader, writer) =
        io::pipe().context("failed to create message pipe")?;
    let reader = MessageReader(BufReader::new(reader));
    let writer = MessageWriter(writer);

    Ok((reader, writer))
}
