use anyhow::Error;
use crossbeam_channel::Receiver;
use emma::comm::*;
use fehler::throws;
use fs_err as fs;
use std::io;
use std::thread;

#[throws]
fn response_thread(rx: Receiver<Response>) {
    let stdout = io::stdout();
    let mut stdout_handle = stdout.lock();

    loop {
        let resp = rx.recv()?;

        send(&resp, &mut stdout_handle)?;

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
    loop {
        let req: Request = recv(&mut stdin_handle)?;

        match req {
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
