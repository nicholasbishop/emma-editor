use anyhow::Error;
use fehler::throws;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;

type Id = [u8; 8];

#[derive(Deserialize, Serialize, Debug)]
struct ReadFileRequest {
    id: Id,
    path: PathBuf,
}

#[derive(Deserialize, Serialize, Debug)]
enum ReadFileResponseBody {
    TotalSize(usize),
    Data(Vec<u8>),
}

#[derive(Deserialize, Serialize, Debug)]
struct ReadFileResponse {
    id: Id,
    body: ReadFileResponseBody,
}

#[derive(Deserialize, Serialize, Debug)]
enum Request {
    Ping,
    ReadFile(ReadFileRequest),
}

#[derive(Deserialize, Serialize, Debug)]
enum Response {
    Pong,
    ReadFile(ReadFileResponse),
}

#[throws]
fn respond(resp: Response) {
    // TODO for now just write to stdout and flush, may need to get
    // more complicated eventually

    let msg = bincode::serialize(&resp)?;

    // TODO: could move this lock out
    let stdout = io::stdout();
    let mut stdout_handle = stdout.lock();

    stdout_handle.write_all(&msg)?;
    stdout_handle.flush()?;
}

fn main() -> Result<(), Error> {
    // TODO: split response work up into threads. Maybe one thread for
    // all file IO (or a file IO thread pool), one thread for each
    // long running process?

    let stdin = io::stdin();
    let mut stdin_handle = stdin.lock();
    let mut len_buf: [u8; 2] = [0; 2];
    let mut msg_buf = Vec::new();
    loop {
        stdin_handle.read_exact(&mut len_buf)?;
        let msg_len = u16::from_le_bytes(len_buf);
        dbg!(msg_len);
        msg_buf.resize(msg_len as usize, 0);
        stdin_handle.read_exact(&mut msg_buf)?;

        let msg = bincode::deserialize(&msg_buf)?;
        match msg {
            Request::Ping => respond(Response::Pong)?,
            Request::ReadFile(req) => {
                let contents = fs::read(req.path)?;
                respond(Response::ReadFile(ReadFileResponse {
                    id: req.id,
                    body: ReadFileResponseBody::TotalSize(contents.len()),
                }))?;
                for chunk in contents.chunks(4096) {
                    respond(Response::ReadFile(ReadFileResponse {
                        id: req.id,
                        body: ReadFileResponseBody::Data(chunk.to_vec()),
                    }))?;
                }
            }
        }
    }
}
