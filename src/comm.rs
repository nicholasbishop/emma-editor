use anyhow::Error;
use fehler::throws;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::PathBuf;

pub type Id = [u8; 8];

#[derive(Deserialize, Serialize, Debug, Eq, PartialEq)]
pub struct ReadFileRequest {
    pub id: Id,
    pub path: PathBuf,
}

#[derive(Deserialize, Serialize, Debug, Eq, PartialEq)]
pub enum ReadFileResponseBody {
    TotalSize(usize),
    Data(Vec<u8>),
}

#[derive(Deserialize, Serialize, Debug, Eq, PartialEq)]
pub struct ReadFileResponse {
    pub id: Id,
    pub body: ReadFileResponseBody,
}

#[derive(Deserialize, Serialize, Debug, Eq, PartialEq)]
pub enum Request {
    Ping,
    ReadFile(ReadFileRequest),
    Stop,
}

#[derive(Deserialize, Serialize, Debug, Eq, PartialEq)]
pub enum Response {
    Pong,
    ReadFile(ReadFileResponse),
    Stop,
}

#[throws]
pub fn send<W, T: ?Sized>(msg: &T, mut writer: W)
where
    W: Write,
    T: Serialize,
{
    bincode::serialize_into(&mut writer, msg)?;
    writer.flush()?;
}

#[throws]
pub fn recv<R, T>(reader: R) -> T
where
    R: Read,
    T: DeserializeOwned,
{
    bincode::deserialize_from(reader)?
}
