//! Minimal parser for Valve's text KeyValues format (`.vdf` / `.acf`).
//!
//! Supports quoted and unquoted tokens, `\"` `\\` `\n` `\t` escapes, `//`
//! line comments, and skips platform conditionals like `[$WIN32]`.
//! Keys are kept in file order; lookups are case-insensitive, matching
//! how Steam itself treats them.

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Str(String),
    Obj(Vec<(String, Value)>),
}

impl Value {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            Value::Obj(_) => None,
        }
    }

    pub fn entries(&self) -> &[(String, Value)] {
        match self {
            Value::Obj(v) => v,
            Value::Str(_) => &[],
        }
    }

    /// First child whose key matches case-insensitively.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.entries()
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v)
    }

    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Value::as_str)
    }

    /// Follow a path of keys, e.g. `["Software", "Valve", "Steam"]`.
    pub fn path(&self, keys: &[&str]) -> Option<&Value> {
        keys.iter().try_fold(self, |v, k| v.get(k))
    }
}

#[derive(Debug, PartialEq)]
pub struct ParseError {
    pub line: usize,
    pub msg: &'static str,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.msg)
    }
}

impl std::error::Error for ParseError {}

#[derive(Debug, PartialEq)]
enum Tok {
    Str(String),
    Open,
    Close,
}

struct Lexer<'a> {
    s: &'a [u8],
    pos: usize,
    line: usize,
}

impl<'a> Lexer<'a> {
    fn err(&self, msg: &'static str) -> ParseError {
        ParseError { line: self.line, msg }
    }

    fn next(&mut self) -> Result<Option<Tok>, ParseError> {
        loop {
            let Some(&c) = self.s.get(self.pos) else {
                return Ok(None);
            };
            match c {
                b'\n' => {
                    self.line += 1;
                    self.pos += 1;
                }
                c if c.is_ascii_whitespace() => self.pos += 1,
                b'/' if self.s.get(self.pos + 1) == Some(&b'/') => {
                    while self.pos < self.s.len() && self.s[self.pos] != b'\n' {
                        self.pos += 1;
                    }
                }
                b'[' => {
                    // Conditional like [$WIN32]; ignored.
                    while self.pos < self.s.len() && self.s[self.pos] != b']' {
                        self.pos += 1;
                    }
                    self.pos += 1;
                }
                b'{' => {
                    self.pos += 1;
                    return Ok(Some(Tok::Open));
                }
                b'}' => {
                    self.pos += 1;
                    return Ok(Some(Tok::Close));
                }
                b'"' => return self.quoted().map(Some),
                _ => return Ok(Some(self.bare())),
            }
        }
    }

    fn quoted(&mut self) -> Result<Tok, ParseError> {
        self.pos += 1;
        let mut out = Vec::new();
        loop {
            let Some(&c) = self.s.get(self.pos) else {
                return Err(self.err("unterminated string"));
            };
            self.pos += 1;
            match c {
                b'"' => break,
                b'\\' => {
                    let Some(&e) = self.s.get(self.pos) else {
                        return Err(self.err("unterminated escape"));
                    };
                    self.pos += 1;
                    match e {
                        b'n' => out.push(b'\n'),
                        b't' => out.push(b'\t'),
                        b'"' => out.push(b'"'),
                        b'\\' => out.push(b'\\'),
                        // Unknown escape: keep it verbatim (Windows paths in
                        // some files use single backslashes).
                        other => {
                            out.push(b'\\');
                            out.push(other);
                        }
                    }
                }
                b'\n' => {
                    self.line += 1;
                    out.push(c);
                }
                _ => out.push(c),
            }
        }
        Ok(Tok::Str(String::from_utf8_lossy(&out).into_owned()))
    }

    fn bare(&mut self) -> Tok {
        let start = self.pos;
        while let Some(&c) = self.s.get(self.pos) {
            if c.is_ascii_whitespace() || matches!(c, b'{' | b'}' | b'"') {
                break;
            }
            self.pos += 1;
        }
        Tok::Str(String::from_utf8_lossy(&self.s[start..self.pos]).into_owned())
    }
}

/// Parse a whole document. The result is an object holding the top-level
/// key(s), usually exactly one (e.g. `"AppState"`).
pub fn parse(input: &str) -> Result<Value, ParseError> {
    let input = input.strip_prefix('\u{feff}').unwrap_or(input);
    let mut lx = Lexer { s: input.as_bytes(), pos: 0, line: 1 };
    let root = parse_obj(&mut lx, false)?;
    Ok(Value::Obj(root))
}

fn parse_obj(lx: &mut Lexer, nested: bool) -> Result<Vec<(String, Value)>, ParseError> {
    let mut out = Vec::new();
    loop {
        let key = match lx.next()? {
            None if nested => return Err(lx.err("unexpected end of file, missing '}'")),
            None => return Ok(out),
            Some(Tok::Close) if nested => return Ok(out),
            Some(Tok::Close) => return Err(lx.err("unexpected '}'")),
            Some(Tok::Open) => return Err(lx.err("expected key, found '{'")),
            Some(Tok::Str(k)) => k,
        };
        let val = match lx.next()? {
            Some(Tok::Str(v)) => Value::Str(v),
            Some(Tok::Open) => Value::Obj(parse_obj(lx, true)?),
            Some(Tok::Close) | None => return Err(lx.err("key without value")),
        };
        out.push((key, val));
    }
}
