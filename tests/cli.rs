//! End-to-end CLI tests that spawn the actual `slice` binary and pipe
//! data through stdin.

use std::io::Write;
use std::process::{Command, Stdio};

fn run(expr: &str, input: &str) -> (String, String, i32) {
    let bin = env!("CARGO_BIN_EXE_slice");
    let mut child = Command::new(bin)
        .arg(expr)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn binary");
    {
        let mut stdin = child.stdin.take().expect("no stdin");
        stdin.write_all(input.as_bytes()).expect("write stdin");
    }
    let out = child.wait_with_output().expect("wait");
    (
        String::from_utf8(out.stdout).unwrap(),
        String::from_utf8(out.stderr).unwrap(),
        out.status.code().unwrap_or(-1),
    )
}

#[test]
fn drops_first_two_columns() {
    let (stdout, _, code) = run("2:", "a b c d e\n1 2 3 4 5\n");
    assert_eq!(code, 0);
    assert_eq!(stdout, "c d e\n3 4 5\n");
}

#[test]
fn picks_single_column() {
    let (stdout, _, code) = run("1", "a b c\nx y z\n");
    assert_eq!(code, 0);
    assert_eq!(stdout, "b\ny\n");
}

#[test]
fn negative_index() {
    let (stdout, _, code) = run("-1", "a b c\nx y z\n");
    assert_eq!(code, 0);
    assert_eq!(stdout, "c\nz\n");
}

#[test]
fn reverse_with_step() {
    let (stdout, _, code) = run("::-1", "a b c d\n");
    assert_eq!(code, 0);
    assert_eq!(stdout, "d c b a\n");
}

#[test]
fn comma_separated_picks() {
    let (stdout, _, code) = run("0,-1", "a b c d\n1 2 3 4\n");
    assert_eq!(code, 0);
    assert_eq!(stdout, "a d\n1 4\n");
}

#[test]
fn handles_blank_and_irregular_lines() {
    // A blank line yields a blank line; short lines drop missing columns.
    let (stdout, _, code) = run("2:", "a b\n\na b c d\n");
    assert_eq!(code, 0);
    assert_eq!(stdout, "\n\nc d\n");
}

#[test]
fn rejects_bad_expression() {
    let (_, stderr, code) = run("nope", "a b c\n");
    assert_eq!(code, 2);
    assert!(stderr.contains("parse error"), "stderr was: {stderr}");
}

#[test]
fn rejects_missing_argument() {
    let bin = env!("CARGO_BIN_EXE_slice");
    let out = Command::new(bin).output().expect("run");
    assert_eq!(out.status.code().unwrap_or(-1), 2);
    assert!(String::from_utf8_lossy(&out.stderr).contains("usage"));
}
