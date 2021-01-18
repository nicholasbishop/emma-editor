use serde::{Deserialize, Serialize};
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
