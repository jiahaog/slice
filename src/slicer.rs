//! Apply a parsed slice expression to a sequence of columns,
//! mirroring Python's indexing/slicing semantics for lists/strings.

use crate::parser::Expr;

/// Apply `expr` to `cols`, returning the selected items in order.
///
/// * `Expr::Index` selects a single element. Out-of-range indices are dropped
///   silently (so missing columns don't blow up the whole pipeline). Python
///   would raise `IndexError`; we're more forgiving because rows in real data
///   are often ragged.
/// * `Expr::Slice` follows Python's `slice.indices` semantics, including
///   support for negative steps and reversed traversal.
pub fn apply<'a>(expr: &Expr, cols: &[&'a str]) -> Vec<&'a str> {
    let len = cols.len() as i64;
    let mut out = Vec::new();

    match *expr {
        Expr::Index(i) => {
            let normalized = if i < 0 { i + len } else { i };
            if normalized >= 0 && normalized < len {
                out.push(cols[normalized as usize]);
            }
        }
        Expr::Slice { start, stop, step } => {
            let (mut i, end, step) = slice_indices(len, start, stop, step);
            if step > 0 {
                while i < end {
                    out.push(cols[i as usize]);
                    i += step;
                }
            } else {
                while i > end {
                    out.push(cols[i as usize]);
                    i += step;
                }
            }
        }
    }
    out
}

/// Equivalent to CPython's `slice.indices(length)`. Returns
/// `(start, stop, step)` ready to drive a stepped loop.
///
/// `step` defaults to 1 and may not be 0 (a 0 step is silently coerced to 1
/// to keep the CLI from panicking on bad input).
fn slice_indices(
    length: i64,
    start: Option<i64>,
    stop: Option<i64>,
    step: Option<i64>,
) -> (i64, i64, i64) {
    let step = match step {
        Some(0) => 1, // be permissive; Python raises ValueError here
        Some(s) => s,
        None => 1,
    };

    let (lower, upper) = if step > 0 { (0, length) } else { (-1, length - 1) };

    let clamp = |x: i64| -> i64 {
        let mut v = if x < 0 { x + length } else { x };
        if v < lower {
            v = lower;
        }
        if v > upper {
            v = upper;
        }
        v
    };

    let start = match start {
        Some(s) => clamp(s),
        None => {
            if step > 0 {
                0
            } else {
                length - 1
            }
        }
    };
    let stop = match stop {
        Some(s) => clamp(s),
        None => {
            if step > 0 {
                length
            } else {
                -1
            }
        }
    };

    (start, stop, step)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    fn run<'a>(expr: &str, line: &'a str) -> Vec<&'a str> {
        let e = parse(expr).expect("parse failed");
        let cols: Vec<&str> = line.split_whitespace().collect();
        apply(&e, &cols)
    }

    #[test]
    fn single_index_positive() {
        assert_eq!(run("0", "a b c d"), vec!["a"]);
        assert_eq!(run("2", "a b c d"), vec!["c"]);
    }

    #[test]
    fn single_index_negative() {
        assert_eq!(run("-1", "a b c d"), vec!["d"]);
        assert_eq!(run("-4", "a b c d"), vec!["a"]);
    }

    #[test]
    fn out_of_range_index_drops() {
        let empty: Vec<&str> = vec![];
        assert_eq!(run("9", "a b c"), empty);
        assert_eq!(run("-9", "a b c"), empty);
    }

    #[test]
    fn open_slices() {
        assert_eq!(run(":", "a b c"), vec!["a", "b", "c"]);
        assert_eq!(run("1:", "a b c d"), vec!["b", "c", "d"]);
        assert_eq!(run(":2", "a b c d"), vec!["a", "b"]);
        assert_eq!(run("1:3", "a b c d"), vec!["b", "c"]);
    }

    #[test]
    fn negative_bounds() {
        assert_eq!(run("-2:", "a b c d"), vec!["c", "d"]);
        assert_eq!(run(":-1", "a b c d"), vec!["a", "b", "c"]);
        assert_eq!(run("-3:-1", "a b c d"), vec!["b", "c"]);
    }

    #[test]
    fn step_slices() {
        assert_eq!(run("::2", "a b c d e"), vec!["a", "c", "e"]);
        assert_eq!(run("1::2", "a b c d e"), vec!["b", "d"]);
        assert_eq!(run("::-1", "a b c d"), vec!["d", "c", "b", "a"]);
        assert_eq!(run("4:0:-1", "a b c d e"), vec!["e", "d", "c", "b"]);
        assert_eq!(run("::-2", "a b c d e"), vec!["e", "c", "a"]);
    }

    #[test]
    fn empty_slice_results() {
        let empty: Vec<&str> = vec![];
        assert_eq!(run("3:1", "a b c d"), empty);
        assert_eq!(run("1:1", "a b c d"), empty);
        assert_eq!(run("1:3:-1", "a b c d"), empty);
    }

    #[test]
    fn out_of_bound_slices_clamp() {
        assert_eq!(run("0:100", "a b c"), vec!["a", "b", "c"]);
        assert_eq!(run("-100:2", "a b c"), vec!["a", "b"]);
        assert_eq!(run("-100:", "a b c"), vec!["a", "b", "c"]);
    }
}
