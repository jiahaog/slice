mod parser;
mod slicer;

use std::io::{self, BufRead, BufWriter, Write};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let expr = match args.as_slice() {
        [e] => e.clone(),
        _ => {
            eprintln!("usage: slice <expr>");
            eprintln!("       <expr> is a Python-style slice, e.g. '2', '2:', ':-1', '::-1', '1:8:2'");
            return ExitCode::from(2);
        }
    };

    let expr = match parser::parse(&expr) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("slice: {e}");
            return ExitCode::from(2);
        }
    };

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                eprintln!("slice: read error: {e}");
                return ExitCode::from(1);
            }
        };
        let cols: Vec<&str> = line.split_whitespace().collect();
        let selected = slicer::apply(&expr, &cols);
        // join with single space; print empty line if nothing matched
        let mut first = true;
        for s in selected {
            if !first {
                out.write_all(b" ").ok();
            }
            out.write_all(s.as_bytes()).ok();
            first = false;
        }
        out.write_all(b"\n").ok();
    }
    ExitCode::SUCCESS
}
