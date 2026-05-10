//! Parity tests: run a battery of (expression, input) cases through both
//! the Rust `slice` binary and the Python reference implementation, and
//! assert that their stdout matches exactly.
//!
//! These tests are skipped (with a printed warning) if `python3` isn't
//! available on PATH.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

const REFERENCE: &str = "tests/reference.py";

fn python3_available() -> bool {
    Command::new("python3")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn run_cmd(mut cmd: Command, input: &str) -> (String, String, i32) {
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn");
    child
        .stdin
        .take()
        .expect("no stdin")
        .write_all(input.as_bytes())
        .expect("write stdin");
    let out = child.wait_with_output().expect("wait");
    (
        String::from_utf8(out.stdout).unwrap(),
        String::from_utf8(out.stderr).unwrap(),
        out.status.code().unwrap_or(-1),
    )
}

fn run_rust(expr: &str, input: &str) -> (String, String, i32) {
    let bin = env!("CARGO_BIN_EXE_slice");
    let mut cmd = Command::new(bin);
    cmd.arg(expr);
    run_cmd(cmd, input)
}

fn run_python(expr: &str, input: &str) -> (String, String, i32) {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push(REFERENCE);
    let mut cmd = Command::new("python3");
    cmd.arg(path).arg(expr);
    run_cmd(cmd, input)
}

/// Sample inputs covering common shapes: short, long, ragged, blank lines,
/// numeric-looking columns.
const INPUTS: &[&str] = &[
    "a b c d e\n1 2 3 4 5\n",
    "one two three\nfoo bar baz qux\n",
    "x\n",
    "\n",
    "  leading spaces\n",
    "trailing spaces   \n",
    "a b\n\na b c d\nfoo\n",
    "alpha beta gamma delta epsilon zeta eta theta\n",
];

/// Expressions that are valid Python list slicing AND valid in our parser.
const EXPRS: &[&str] = &[
    // Pure indices
    "0", "1", "2", "-1", "-2", "-3",
    // Open slices
    ":", "1:", ":2", ":-1", "-2:", "1:3", "-3:-1",
    // Stepped
    "::1", "::2", "::-1", "::-2", "1::2", "::3",
    "0:5:2", "5:0:-1", "-1::-1", "-1:-4:-1",
    // Fully clamped / empty
    "10:100", "-100:0", "3:1", "1:1",
];

#[test]
fn parity_with_python_reference() {
    if !python3_available() {
        eprintln!("python3 not found; skipping parity test");
        return;
    }

    let mut failures: Vec<String> = Vec::new();
    let mut ran = 0usize;

    for input in INPUTS {
        for expr in EXPRS {
            let (rs_out, _rs_err, rs_code) = run_rust(expr, input);
            let (py_out, _py_err, py_code) = run_python(expr, input);
            ran += 1;
            // We don't compare exit codes for in-range expressions because
            // both should be 0; we DO compare stdout exactly.
            if rs_code != 0 || py_code != 0 {
                failures.push(format!(
                    "non-zero exit for expr={expr:?} input={input:?}: rust={rs_code} python={py_code}"
                ));
                continue;
            }
            if rs_out != py_out {
                failures.push(format!(
                    "mismatch for expr={expr:?} input={input:?}\n  rust:   {rs_out:?}\n  python: {py_out:?}"
                ));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "parity failures ({}/{} cases):\n{}",
        failures.len(),
        ran,
        failures.join("\n")
    );
}

/// Anything we reject for syntactic reasons should also be rejected by
/// Python (either by `compile` raising SyntaxError, or by list subscript
/// raising TypeError at eval time).
#[test]
fn parity_rejects_invalid_expressions() {
    if !python3_available() {
        eprintln!("python3 not found; skipping parity test");
        return;
    }

    // Each of these should produce a non-zero exit from BOTH binaries.
    let bad_exprs = ["abc", "0,1", "0,2:4", "1::2:3", "1 2", ""];
    let input = "a b c d\n";
    for expr in bad_exprs {
        let (_, _, rs_code) = run_rust(expr, input);
        let (_, _, py_code) = run_python(expr, input);
        assert!(
            rs_code != 0 && py_code != 0,
            "expected both to reject {expr:?}, got rust={rs_code} python={py_code}"
        );
    }
}
