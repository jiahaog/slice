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
            let normalized = if i >= 0 {
                i
            } else if i >= -len {
                i + len
            } else {
                return out;
            };
            if normalized >= 0 && normalized < len {
                out.push(cols[normalized as usize]);
            }
        }
        Expr::Slice { start, stop, step } => {
            let (mut i, end, step) = slice_indices(len, start, stop, step);
            if step > 0 {
                while i < end {
                    out.push(cols[i as usize]);
                    let Some(next) = i.checked_add(step) else {
                        break;
                    };
                    i = next;
                }
            } else {
                while i > end {
                    out.push(cols[i as usize]);
                    let Some(next) = i.checked_add(step) else {
                        break;
                    };
                    i = next;
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

    let (lower, upper) = if step > 0 {
        (0, length)
    } else {
        (-1, length - 1)
    };

    let clamp = |x: i64| -> i64 {
        if x < 0 {
            if x < lower - length {
                lower
            } else {
                x + length
            }
        } else if x > upper {
            upper
        } else {
            x
        }
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
    use quickcheck::QuickCheck;

    type GeneratedExprParts = (bool, i64, Option<i64>, Option<i64>, Option<i64>);

    fn render_generated_expr((is_index, index, start, stop, step): GeneratedExprParts) -> String {
        if is_index {
            return index.to_string();
        }

        let start = start.map(|n| n.to_string()).unwrap_or_default();
        let stop = stop.map(|n| n.to_string()).unwrap_or_default();
        match step {
            Some(step) => format!("{start}:{stop}:{step}"),
            None => format!("{start}:{stop}"),
        }
    }

    fn run<'a>(expr: &str, line: &'a str) -> Vec<&'a str> {
        let e = parse(expr).expect("parse failed");
        let cols: Vec<&str> = line.split_whitespace().collect();
        apply(&e, &cols)
    }

    fn output_is_subset_of_input(expr: &Expr, cols: &[String]) -> bool {
        let col_refs: Vec<&str> = cols.iter().map(String::as_str).collect();
        let selected = apply(expr, &col_refs);

        selected.len() <= col_refs.len() && selected.iter().all(|item| col_refs.contains(item))
    }

    #[test]
    fn quickcheck_generated_expressions_parse_and_apply() {
        fn prop(expr: GeneratedExprParts, cols: Vec<String>) -> bool {
            let rendered = render_generated_expr(expr);
            let Ok(parsed) = parse(&rendered) else {
                return false;
            };
            output_is_subset_of_input(&parsed, &cols)
        }

        QuickCheck::new()
            .tests(1000)
            .quickcheck(prop as fn(GeneratedExprParts, Vec<String>) -> bool);
    }

    #[test]
    fn quickcheck_arbitrary_expressions_do_not_panic() {
        fn prop(expr: String, cols: Vec<String>) -> bool {
            let Ok(parsed) = parse(&expr) else {
                return true;
            };
            output_is_subset_of_input(&parsed, &cols)
        }

        QuickCheck::new()
            .tests(1000)
            .quickcheck(prop as fn(String, Vec<String>) -> bool);
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
    fn extreme_integer_bounds_do_not_overflow() {
        let empty: Vec<&str> = vec![];
        assert_eq!(run(&i64::MIN.to_string(), "a b c"), empty);
        assert_eq!(run(&format!("{}:", i64::MIN), "a b c"), vec!["a", "b", "c"]);
        assert_eq!(run(&format!("1::{}", i64::MAX), "a b c"), vec!["b"]);
        assert_eq!(run(&format!("1::{}", i64::MIN), "a b c"), vec!["b"]);
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
