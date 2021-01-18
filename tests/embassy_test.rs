use anyhow::Error;
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

    let mut proc = Command::new("target/debug/embassy")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    let status = proc.wait()?;
    assert!(status.success());
}
