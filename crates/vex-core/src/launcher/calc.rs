//! Safe calculator: tokenizer plus recursive-descent parser over `f64`.
//!
//! Supports `+ - * / % ^`, parentheses, unary minus/plus, decimals, `pi`, `e`, and a fixed set of
//! one-argument functions. Input length, nesting depth and token count are capped so no input can
//! hang or overflow the stack. Nothing is ever executed.

use std::fmt;

const MAX_LEN: usize = 200;
const MAX_DEPTH: usize = 32;

#[derive(Debug, Clone, PartialEq)]
pub enum CalcError {
    Empty,
    TooLong,
    TooDeep,
    Unexpected(String),
    UnknownName(String),
    DivideByZero,
    NotFinite,
}

impl fmt::Display for CalcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CalcError::Empty => write!(f, "Type a calculation, e.g. 245 * 18"),
            CalcError::TooLong => write!(f, "That expression is too long"),
            CalcError::TooDeep => write!(f, "Too many nested brackets"),
            CalcError::Unexpected(s) => write!(f, "Unexpected {s}"),
            CalcError::UnknownName(s) => write!(f, "Unknown name \"{s}\""),
            CalcError::DivideByZero => write!(f, "Cannot divide by zero"),
            CalcError::NotFinite => write!(f, "The result is not a finite number"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Name(String),
    Op(char),
    LParen,
    RParen,
}

fn tokenize(src: &str) -> Result<Vec<Tok>, CalcError> {
    let chars: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            ' ' | '\t' => i += 1,
            '0'..='9' | '.' => {
                let start = i;
                let mut dots = 0;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.' || (chars[i] == ',' && chars.get(i + 1).is_some_and(|n| n.is_ascii_digit()))) {
                    if chars[i] == '.' {
                        dots += 1;
                    }
                    i += 1;
                }
                if dots > 1 {
                    return Err(CalcError::Unexpected("number".into()));
                }
                // "1,000" is a thousands separator
                let text: String = chars[start..i].iter().filter(|c| **c != ',').collect();
                let n: f64 = text.parse().map_err(|_| CalcError::Unexpected(format!("\"{text}\"")))?;
                out.push(Tok::Num(n));
            }
            '+' | '-' | '*' | '/' | '%' | '^' => {
                out.push(Tok::Op(c));
                i += 1;
            }
            '×' | 'x' | 'X' if matches!(out.last(), Some(Tok::Num(_)) | Some(Tok::RParen)) && !chars.get(i + 1).is_some_and(|n| n.is_alphabetic()) => {
                out.push(Tok::Op('*'));
                i += 1;
            }
            '÷' => {
                out.push(Tok::Op('/'));
                i += 1;
            }
            '(' => {
                out.push(Tok::LParen);
                i += 1;
            }
            ')' => {
                out.push(Tok::RParen);
                i += 1;
            }
            c if c.is_alphabetic() => {
                let start = i;
                while i < chars.len() && chars[i].is_alphabetic() {
                    i += 1;
                }
                out.push(Tok::Name(chars[start..i].iter().collect::<String>().to_lowercase()));
            }
            other => return Err(CalcError::Unexpected(format!("character \"{other}\""))),
        }
    }
    Ok(out)
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
    depth: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        self.pos += 1;
        t
    }

    // expr := term (('+'|'-') term)*
    fn expr(&mut self) -> Result<f64, CalcError> {
        let mut v = self.term()?;
        while let Some(Tok::Op(c @ ('+' | '-'))) = self.peek().cloned() {
            self.pos += 1;
            let r = self.term()?;
            v = if c == '+' { v + r } else { v - r };
        }
        Ok(v)
    }

    // term := unary (('*'|'/'|'%') unary)*
    fn term(&mut self) -> Result<f64, CalcError> {
        let mut v = self.unary()?;
        while let Some(Tok::Op(c @ ('*' | '/' | '%'))) = self.peek().cloned() {
            self.pos += 1;
            let r = self.unary()?;
            v = match c {
                '*' => v * r,
                '/' => {
                    if r == 0.0 {
                        return Err(CalcError::DivideByZero);
                    }
                    v / r
                }
                _ => {
                    if r == 0.0 {
                        return Err(CalcError::DivideByZero);
                    }
                    v % r
                }
            };
        }
        Ok(v)
    }

    // unary := ('-'|'+') unary | power
    fn unary(&mut self) -> Result<f64, CalcError> {
        match self.peek() {
            Some(Tok::Op('-')) => {
                self.pos += 1;
                self.enter()?;
                let v = self.unary();
                self.depth -= 1;
                Ok(-v?)
            }
            Some(Tok::Op('+')) => {
                self.pos += 1;
                self.enter()?;
                let v = self.unary();
                self.depth -= 1;
                v
            }
            _ => self.power(),
        }
    }

    // power := atom ('^' unary)?   (right associative)
    fn power(&mut self) -> Result<f64, CalcError> {
        let base = self.atom()?;
        if let Some(Tok::Op('^')) = self.peek() {
            self.pos += 1;
            self.enter()?;
            let exp = self.unary();
            self.depth -= 1;
            return Ok(base.powf(exp?));
        }
        Ok(base)
    }

    fn enter(&mut self) -> Result<(), CalcError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(CalcError::TooDeep);
        }
        Ok(())
    }

    fn atom(&mut self) -> Result<f64, CalcError> {
        match self.next() {
            Some(Tok::Num(n)) => Ok(n),
            Some(Tok::LParen) => {
                self.enter()?;
                let v = self.expr();
                self.depth -= 1;
                let v = v?;
                match self.next() {
                    Some(Tok::RParen) => Ok(v),
                    _ => Err(CalcError::Unexpected("end: missing )".into())),
                }
            }
            Some(Tok::Name(n)) => match n.as_str() {
                "pi" => Ok(std::f64::consts::PI),
                "e" => Ok(std::f64::consts::E),
                f @ ("sqrt" | "abs" | "sin" | "cos" | "tan" | "ln" | "log" | "round" | "floor" | "ceil") => {
                    if !matches!(self.next(), Some(Tok::LParen)) {
                        return Err(CalcError::Unexpected(format!("end: {f} needs brackets, e.g. {f}(4)")));
                    }
                    self.enter()?;
                    let v = self.expr();
                    self.depth -= 1;
                    let v = v?;
                    if !matches!(self.next(), Some(Tok::RParen)) {
                        return Err(CalcError::Unexpected("end: missing )".into()));
                    }
                    Ok(match f {
                        "sqrt" => v.sqrt(),
                        "abs" => v.abs(),
                        "sin" => v.sin(),
                        "cos" => v.cos(),
                        "tan" => v.tan(),
                        "ln" => v.ln(),
                        "log" => v.log10(),
                        "round" => v.round(),
                        "floor" => v.floor(),
                        _ => v.ceil(),
                    })
                }
                other => Err(CalcError::UnknownName(other.into())),
            },
            Some(Tok::Op(c)) => Err(CalcError::Unexpected(format!("\"{c}\""))),
            Some(Tok::RParen) => Err(CalcError::Unexpected("\")\"".into())),
            None => Err(CalcError::Unexpected("end of input".into())),
        }
    }
}

/// Evaluate an arithmetic expression.
pub fn evaluate(src: &str) -> Result<f64, CalcError> {
    let src = src.trim();
    if src.is_empty() {
        return Err(CalcError::Empty);
    }
    if src.chars().count() > MAX_LEN {
        return Err(CalcError::TooLong);
    }
    let toks = tokenize(src)?;
    let mut p = Parser { toks, pos: 0, depth: 0 };
    let v = p.expr()?;
    if p.pos < p.toks.len() {
        return Err(CalcError::Unexpected(match &p.toks[p.pos] {
            Tok::RParen => "\")\"".into(),
            Tok::Op(c) => format!("\"{c}\""),
            Tok::Num(_) => "number".into(),
            Tok::Name(n) => format!("\"{n}\""),
            Tok::LParen => "\"(\"".into(),
        }));
    }
    if !v.is_finite() {
        return Err(CalcError::NotFinite);
    }
    Ok(v)
}

/// Format a result without float noise: integers print plainly, others to at most 10 significant digits.
pub fn format_number(v: f64) -> String {
    if v == v.trunc() && v.abs() < 1e15 {
        return format!("{}", v as i64);
    }
    let s = format!("{:.10}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.len() > 22 {
        return format!("{v:e}");
    }
    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(s: &str) -> f64 {
        evaluate(s).unwrap()
    }

    #[test]
    fn basic_arithmetic_and_precedence() {
        assert_eq!(ev("245 * 18"), 4410.0);
        assert_eq!(ev("2 + 3 * 4"), 14.0);
        assert_eq!(ev("(2 + 3) * 4"), 20.0);
        assert_eq!(ev("10 - 4 - 3"), 3.0);
        assert_eq!(ev("100 / 8"), 12.5);
        assert_eq!(ev("17 % 5"), 2.0);
    }

    #[test]
    fn power_is_right_associative_and_unary_works() {
        assert_eq!(ev("2 ^ 3 ^ 2"), 512.0);
        assert_eq!(ev("-2 ^ 2"), -4.0);
        assert_eq!(ev("--3"), 3.0);
        assert_eq!(ev("2 * -3"), -6.0);
        assert_eq!(ev("+5"), 5.0);
    }

    #[test]
    fn decimals_separators_and_symbols() {
        assert!((ev("0.1 + 0.2") - 0.3).abs() < 1e-12);
        assert_eq!(ev("1,000 + 1"), 1001.0);
        assert_eq!(ev("6 × 7"), 42.0);
        assert_eq!(ev("6 x 7"), 42.0);
        assert_eq!(ev("84 ÷ 2"), 42.0);
        assert_eq!(ev(".5 * 4"), 2.0);
    }

    #[test]
    fn functions_and_constants() {
        assert_eq!(ev("sqrt(16)"), 4.0);
        assert_eq!(ev("abs(-3)"), 3.0);
        assert!((ev("pi") - std::f64::consts::PI).abs() < 1e-12);
        assert_eq!(ev("round(2.6)"), 3.0);
        assert_eq!(ev("log(1000)"), 3.0);
        assert!((ev("2 * pi * 3") - 6.0 * std::f64::consts::PI).abs() < 1e-9);
    }

    #[test]
    fn errors_are_reported_not_panicked() {
        assert_eq!(evaluate(""), Err(CalcError::Empty));
        assert_eq!(evaluate("1 / 0"), Err(CalcError::DivideByZero));
        assert_eq!(evaluate("5 % 0"), Err(CalcError::DivideByZero));
        assert!(matches!(evaluate("2 +"), Err(CalcError::Unexpected(_))));
        assert!(matches!(evaluate("(2 + 3"), Err(CalcError::Unexpected(_))));
        assert!(matches!(evaluate("2 + 3)"), Err(CalcError::Unexpected(_))));
        assert!(matches!(evaluate("foo(2)"), Err(CalcError::UnknownName(_))));
        assert!(matches!(evaluate("sqrt 4"), Err(CalcError::Unexpected(_))));
        assert!(matches!(evaluate("1.2.3"), Err(CalcError::Unexpected(_))));
        assert_eq!(evaluate("sqrt(-1)"), Err(CalcError::NotFinite));
        assert_eq!(evaluate("10 ^ 1000"), Err(CalcError::NotFinite));
    }

    #[test]
    fn rejects_anything_that_is_not_arithmetic() {
        for bad in ["system('calc')", "1; DROP TABLE", "`ls`", "$(whoami)", "a = 3", "__import__", "2 + \"3\"", "{}", "[1]", "1 & 1"] {
            assert!(evaluate(bad).is_err(), "{bad} must be rejected");
        }
    }

    #[test]
    fn limits_protect_against_hostile_input() {
        let long = "1+".repeat(150) + "1";
        assert_eq!(evaluate(&long), Err(CalcError::TooLong));
        let deep = "(".repeat(60) + "1" + &")".repeat(60);
        assert_eq!(evaluate(&deep), Err(CalcError::TooDeep));
        let negs = "-".repeat(150) + "1";
        assert!(evaluate(&negs).is_err());
    }

    #[test]
    fn formatting_hides_float_noise() {
        assert_eq!(format_number(4410.0), "4410");
        assert_eq!(format_number(0.1 + 0.2), "0.3");
        assert_eq!(format_number(12.5), "12.5");
        assert_eq!(format_number(-7.0), "-7");
        assert_eq!(format_number(1.0 / 3.0), "0.3333333333");
    }
}
