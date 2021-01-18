use anyhow::Error;
use crossbeam_channel::Receiver;
use fehler::throws;
use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::{fs, thread};

type Id = [u8; 8];

#[derive(Deserialize, Serialize, Debug, Eq, PartialEq)]
struct ReadFileRequest {
    id: Id,
    path: PathBuf,
}

#[derive(Deserialize, Serialize, Debug, Eq, PartialEq)]
enum ReadFileResponseBody {
    TotalSize(usize),
    Data(Vec<u8>),
}

#[derive(Deserialize, Serialize, Debug, Eq, PartialEq)]
struct ReadFileResponse {
    id: Id,
    body: ReadFileResponseBody,
}

#[derive(Deserialize, Serialize, Debug, Eq, PartialEq)]
enum Request {
    Ping,
    ReadFile(ReadFileRequest),
    Stop,
}

#[derive(Deserialize, Serialize, Debug, Eq, PartialEq)]
enum Response {
    Pong,
    ReadFile(ReadFileResponse),
    Stop,
}

#[throws]
fn response_thread(rx: Receiver<Response>) {
    let stdout = io::stdout();
    let mut stdout_handle = stdout.lock();

    loop {
        let resp = rx.recv()?;
        let msg = bincode::serialize(&resp)?;

        stdout_handle.write_all(&msg)?;
        stdout_handle.flush()?;

        if resp == Response::Stop {
            break;
        }
    }
}

#[throws]
fn main() {
    // TODO: split response work up into threads. Maybe one thread for
    // all file IO (or a file IO thread pool), one thread for each
    // long running process?

    let (resp_tx, resp_rx) = crossbeam_channel::unbounded();

    let respond = |resp| resp_tx.send(resp);

    let response_thread_handle = thread::spawn(|| response_thread(resp_rx));

    let stdin = io::stdin();
    let mut stdin_handle = stdin.lock();
    let mut len_buf: [u8; 2] = [0; 2];
    let mut msg_buf = Vec::new();
    loop {
        stdin_handle.read_exact(&mut len_buf)?;
        let msg_len = u16::from_le_bytes(len_buf);
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
            Request::Stop => {
                respond(Response::Stop)?;
                break;
            }
        }
    }

    response_thread_handle.join().unwrap()?;
}
