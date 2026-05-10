//! Apply parsed slice expressions to a sequence of columns,
//! mirroring Python's indexing/slicing semantics.

use crate::parser::Part;

/// Apply the given parts to `cols`, returning the selected items in order.
///
/// * `Part::Index` selects a single element. Out-of-range indices are dropped
///   silently (so missing columns don't blow up the whole pipeline).
/// * `Part::Slice` follows Python's `slice.indices` semantics, including
///   support for negative steps and reversed traversal.
pub fn apply<'a>(parts: &[Part], cols: &[&'a str]) -> Vec<&'a str> {
    let len = cols.len() as i64;
    let mut out = Vec::new();

    for part in parts {
        match *part {
            Part::Index(i) => {
                let normalized = if i < 0 { i + len } else { i };
                if normalized >= 0 && normalized < len {
                    out.push(cols[normalized as usize]);
                }
            }
            Part::Slice { start, stop, step } => {
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

    let clamp = |x: i64, default_lo: i64| -> i64 {
        let mut v = if x < 0 { x + length } else { x };
        if v < lower {
            v = lower;
        }
        if v > upper {
            v = upper;
        }
        // The default_lo isn't used when x is provided; kept for symmetry.
        let _ = default_lo;
        v
    };

    let start = match start {
        Some(s) => clamp(s, 0),
        None => {
            if step > 0 {
                0
            } else {
                length - 1
            }
        }
    };
    let stop = match stop {
        Some(s) => clamp(s, 0),
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

    fn cols<'a>(s: &'a str) -> Vec<&'a str> {
        s.split_whitespace().collect()
    }

    fn run<'a>(expr: &str, line: &'a str) -> Vec<&'a str> {
        let parts = parse(expr).expect("parse failed");
        let c = cols(line);
        // Re-borrow into a Vec of &str tied to `line`'s lifetime.
        let owned: Vec<&str> = c;
        apply(&parts, &owned)
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
    fn comma_lists_concatenate() {
        assert_eq!(run("0,2,4", "a b c d e"), vec!["a", "c", "e"]);
        assert_eq!(run("0,2:4", "a b c d e"), vec!["a", "c", "d"]);
        assert_eq!(run("-1,0", "a b c"), vec!["c", "a"]);
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
