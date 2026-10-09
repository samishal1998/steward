//! Conservative interpretation for inventory. Unrecognized syntax is preserved
//! in the native observation and marks conflict coverage partial.
use crate::error::{Diagnostic, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    pub hosts: Vec<String>,
    pub paths: Vec<String>,
    pub any_host: bool,
}
impl Default for Match {
    fn default() -> Self {
        Self {
            hosts: Vec::new(),
            paths: Vec::new(),
            any_host: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Ident(String),
    Literal(String),
    Open,
    Close,
    And,
    Or,
    Comma,
}

pub fn interpret(rule: &str) -> Result<Match> {
    if rule.len() > 65_536 {
        return Err(unsupported());
    }
    let chars: Vec<char> = rule.chars().collect();
    let mut tokens = Vec::new();
    let mut pos = 0;
    while pos < chars.len() {
        let c = chars[pos];
        if c.is_whitespace() {
            pos += 1;
            continue;
        }
        let token = match c {
            '(' => {
                pos += 1;
                Token::Open
            }
            ')' => {
                pos += 1;
                Token::Close
            }
            ',' => {
                pos += 1;
                Token::Comma
            }
            '&' | '|' if chars.get(pos + 1) == Some(&c) => {
                pos += 2;
                if c == '&' { Token::And } else { Token::Or }
            }
            '`' | '"' => {
                let quote = c;
                pos += 1;
                let start = pos;
                while pos < chars.len() && chars[pos] != quote {
                    if chars[pos].is_control() || chars[pos] == '\\' {
                        return Err(unsupported());
                    }
                    pos += 1;
                }
                if pos == chars.len() {
                    return Err(unsupported());
                }
                let value = chars[start..pos].iter().collect();
                pos += 1;
                Token::Literal(value)
            }
            c if c.is_ascii_alphabetic() => {
                let start = pos;
                pos += 1;
                while pos < chars.len() && (chars[pos].is_ascii_alphanumeric() || chars[pos] == '_')
                {
                    pos += 1;
                }
                Token::Ident(chars[start..pos].iter().collect())
            }
            _ => return Err(unsupported()),
        };
        tokens.push(token);
    }
    let mut parser = Parser { tokens, pos: 0 };
    let value = parser.expression(0)?;
    if parser.pos != parser.tokens.len() {
        return Err(unsupported());
    }
    Ok(value)
}

fn unsupported() -> Diagnostic {
    Diagnostic::new(
        "discovery.unsupported_rule",
        "Native rule is outside the literal Host/Path/PathPrefix subset; matcher coverage is partial",
    )
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}
impl Parser {
    fn expression(&mut self, depth: usize) -> Result<Match> {
        if depth > 32 {
            return Err(unsupported());
        }
        let mut out = self.conjunction(depth + 1)?;
        while self.tokens.get(self.pos) == Some(&Token::Or) {
            self.pos += 1;
            let rhs = self.conjunction(depth + 1)?;
            // Collect potential hosts; do not flatten or discard the original expression.
            out.any_host |= rhs.any_host;
            out.hosts.extend(rhs.hosts);
            out.paths.extend(rhs.paths);
        }
        Ok(out)
    }
    fn conjunction(&mut self, depth: usize) -> Result<Match> {
        let mut out = self.term(depth)?;
        while self.tokens.get(self.pos) == Some(&Token::And) {
            self.pos += 1;
            let rhs = self.term(depth)?;
            out.any_host &= rhs.any_host;
            out.hosts.extend(rhs.hosts);
            out.paths.extend(rhs.paths);
        }
        Ok(out)
    }
    fn term(&mut self, depth: usize) -> Result<Match> {
        if self.tokens.get(self.pos) == Some(&Token::Open) {
            self.pos += 1;
            let value = self.expression(depth + 1)?;
            self.expect(Token::Close)?;
            return Ok(value);
        }
        let Some(Token::Ident(name)) = self.tokens.get(self.pos).cloned() else {
            return Err(unsupported());
        };
        if !matches!(name.as_str(), "Host" | "Path" | "PathPrefix") {
            return Err(unsupported());
        }
        self.pos += 1;
        self.expect(Token::Open)?;
        let mut out = Match::default();
        loop {
            let Some(Token::Literal(value)) = self.tokens.get(self.pos).cloned() else {
                return Err(unsupported());
            };
            if value.is_empty() || value.contains(['*', '{', '}']) {
                return Err(unsupported());
            }
            if name == "Host" {
                out.hosts.push(value);
                out.any_host = false;
            } else {
                if !value.starts_with('/') {
                    return Err(unsupported());
                }
                out.paths.push(if name == "PathPrefix" {
                    format!("{value}*")
                } else {
                    value
                });
            }
            self.pos += 1;
            if self.tokens.get(self.pos) == Some(&Token::Comma) {
                self.pos += 1;
            } else {
                break;
            }
        }
        self.expect(Token::Close)?;
        Ok(out)
    }
    fn expect(&mut self, token: Token) -> Result<()> {
        if self.tokens.get(self.pos) != Some(&token) {
            return Err(unsupported());
        }
        self.pos += 1;
        Ok(())
    }
}
