//! Hand-written parser for Python-style slice expressions.
//!
//! Grammar:
//!     expr  := part ("," part)*
//!     part  := slice | index
//!     index := INT
//!     slice := [INT] ":" [INT] [ ":" [INT] ]
//!     INT   := ["+"|"-"] DIGIT+

use std::fmt;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Part {
    /// A single column index, e.g. `2` or `-1`.
    Index(i64),
    /// A slice, e.g. `2:`, `:5`, `1:8:2`, `::-1`.
    Slice {
        start: Option<i64>,
        stop: Option<i64>,
        step: Option<i64>,
    },
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ParseError {
    pub message: String,
    pub position: usize,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "parse error at position {}: {}", self.position, self.message)
    }
}

impl std::error::Error for ParseError {}

pub fn parse(input: &str) -> Result<Vec<Part>, ParseError> {
    let mut p = Parser::new(input);
    p.parse_expr()
}

struct Parser<'a> {
    input: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input: input.as_bytes(), pos: 0 }
    }

    fn peek(&self) -> Option<u8> {
        self.input.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let c = self.peek()?;
        self.pos += 1;
        Some(c)
    }

    fn skip_ws(&mut self) {
        while let Some(c) = self.peek() {
            if c == b' ' || c == b'\t' {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn err(&self, msg: impl Into<String>) -> ParseError {
        ParseError { message: msg.into(), position: self.pos }
    }

    /// Parse an optional integer (with optional `+` / `-` sign).
    /// Returns Ok(None) if no integer is present at the current position.
    fn parse_int(&mut self) -> Result<Option<i64>, ParseError> {
        self.skip_ws();
        let saved = self.pos;
        let mut start = self.pos;

        if matches!(self.peek(), Some(b'+') | Some(b'-')) {
            self.bump();
        }
        let digit_start = self.pos;
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                self.bump();
            } else {
                break;
            }
        }
        if self.pos == digit_start {
            // No digits; rewind any sign we consumed and report "no int".
            self.pos = saved;
            return Ok(None);
        }
        // Skip a leading '+' so i64::from_str accepts it on all toolchains.
        if self.input[start] == b'+' {
            start += 1;
        }
        let s = std::str::from_utf8(&self.input[start..self.pos])
            .map_err(|_| self.err("invalid utf8 in integer"))?;
        s.parse::<i64>()
            .map(Some)
            .map_err(|e| ParseError { message: format!("invalid integer '{s}': {e}"), position: start })
    }

    fn parse_part(&mut self) -> Result<Part, ParseError> {
        self.skip_ws();
        // Disallow an empty part outright (e.g. ",," or trailing ",").
        if matches!(self.peek(), None | Some(b',')) {
            return Err(self.err("expected index or slice"));
        }

        let first = self.parse_int()?;
        self.skip_ws();

        if self.peek() != Some(b':') {
            // Pure index.
            return match first {
                Some(n) => Ok(Part::Index(n)),
                None => Err(self.err("expected integer index")),
            };
        }

        // Slice; consume first ':'.
        self.bump();
        let stop = self.parse_int()?;
        self.skip_ws();

        let step = if self.peek() == Some(b':') {
            self.bump();
            self.parse_int()?
        } else {
            None
        };

        Ok(Part::Slice { start: first, stop, step })
    }

    fn parse_expr(&mut self) -> Result<Vec<Part>, ParseError> {
        let mut parts = vec![self.parse_part()?];
        loop {
            self.skip_ws();
            match self.peek() {
                Some(b',') => {
                    self.bump();
                    parts.push(self.parse_part()?);
                }
                None => break,
                Some(c) => {
                    return Err(self.err(format!("unexpected character {:?}", c as char)));
                }
            }
        }
        Ok(parts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> Vec<Part> {
        parse(s).expect("parse failed")
    }

    fn slice(start: Option<i64>, stop: Option<i64>, step: Option<i64>) -> Part {
        Part::Slice { start, stop, step }
    }

    #[test]
    fn parses_single_index() {
        assert_eq!(p("0"), vec![Part::Index(0)]);
        assert_eq!(p("42"), vec![Part::Index(42)]);
        assert_eq!(p("-3"), vec![Part::Index(-3)]);
        assert_eq!(p("+7"), vec![Part::Index(7)]);
    }

    #[test]
    fn parses_open_slices() {
        assert_eq!(p(":"), vec![slice(None, None, None)]);
        assert_eq!(p("2:"), vec![slice(Some(2), None, None)]);
        assert_eq!(p(":5"), vec![slice(None, Some(5), None)]);
        assert_eq!(p("1:8"), vec![slice(Some(1), Some(8), None)]);
    }

    #[test]
    fn parses_step_slices() {
        assert_eq!(p("::"), vec![slice(None, None, None)]);
        assert_eq!(p("::2"), vec![slice(None, None, Some(2))]);
        assert_eq!(p("::-1"), vec![slice(None, None, Some(-1))]);
        assert_eq!(p("1:8:2"), vec![slice(Some(1), Some(8), Some(2))]);
        assert_eq!(p("-1::-2"), vec![slice(Some(-1), None, Some(-2))]);
    }

    #[test]
    fn parses_comma_lists() {
        assert_eq!(
            p("0,2,4"),
            vec![Part::Index(0), Part::Index(2), Part::Index(4)]
        );
        assert_eq!(
            p("0,2:4,-1"),
            vec![
                Part::Index(0),
                slice(Some(2), Some(4), None),
                Part::Index(-1),
            ]
        );
    }

    #[test]
    fn ignores_whitespace() {
        assert_eq!(p(" 1 : 4 : 2 "), vec![slice(Some(1), Some(4), Some(2))]);
        assert_eq!(p(" 0 , 2 "), vec![Part::Index(0), Part::Index(2)]);
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse("").is_err());
        assert!(parse("abc").is_err());
        assert!(parse("1::2:3").is_err()); // four sections
        assert!(parse(",").is_err());
        assert!(parse("1,").is_err());
        assert!(parse(",1").is_err());
        assert!(parse("1 2").is_err());
    }
}
