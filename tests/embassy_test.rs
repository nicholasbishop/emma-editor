use anyhow::Error;
use emma::comm::*;
use fehler::throws;
use std::process::{Command, Stdio};

#[throws]
fn build_embassy() {
    let status = Command::new("cargo")
        .args(&["build", "--bin", "embassy"])
        .status()?;
    assert!(status.success());
}

#[test]
#[throws]
fn ping_and_stop() {
    build_embassy()?;

    let mut child = Command::new("target/debug/embassy")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = child.stdout.take().unwrap();

    // Send ping, expect pong.
    send(&Request::Ping, &mut stdin)?;
    let resp: Response = recv(&mut stdout)?;
    assert_eq!(resp, Response::Pong);

    // Send stop, expect stop.
    send(&Request::Stop, &mut stdin)?;
    let resp: Response = recv(&mut stdout)?;
    assert_eq!(resp, Response::Stop);

    // Wait for child to exit.
    let status = child.wait()?;
    assert!(status.success());
}
