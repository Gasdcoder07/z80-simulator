//! Tokenizer for the assembler's source syntax.
//!
//! The syntax follows Oshonsoft's Z80 Simulator IDE so that course material
//! written for it assembles unchanged: `;` comments, labels with an optional
//! colon, and constants in character, hex, binary and decimal form with an
//! optional `#` prefix.

use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Token {
    /// A label, mnemonic, register name, directive or symbol reference.
    /// Stored uppercased, since the syntax is case-insensitive.
    Ident(String),
    Number(i64),
    /// A quoted run of characters, used by `DB`.
    Text(String),
    Comma,
    Colon,
    LParen,
    RParen,
    Plus,
    Minus,
    Star,
    Slash,
    /// `&` — bitwise and.
    Ampersand,
    /// `|` — bitwise or.
    Pipe,
    /// `^` — bitwise xor.
    Caret,
    /// `~` — bitwise not.
    Tilde,
    /// `<` — low byte of the following expression.
    Less,
    /// `>` — high byte of the following expression.
    Greater,
    /// `<<`
    ShiftLeft,
    /// `>>`
    ShiftRight,
    /// `=` used as a synonym for `EQU`.
    Equals,
    /// `$` or `PC` — the current assembly address.
    Here,
    /// A lone `'`, which only ever appears in `AF'`.
    Apostrophe,
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::Ident(name) => write!(f, "{name}"),
            Token::Number(value) => write!(f, "{value}"),
            Token::Text(text) => write!(f, "\"{text}\""),
            Token::Comma => f.write_str(","),
            Token::Colon => f.write_str(":"),
            Token::LParen => f.write_str("("),
            Token::RParen => f.write_str(")"),
            Token::Plus => f.write_str("+"),
            Token::Minus => f.write_str("-"),
            Token::Star => f.write_str("*"),
            Token::Slash => f.write_str("/"),
            Token::Ampersand => f.write_str("&"),
            Token::Pipe => f.write_str("|"),
            Token::Caret => f.write_str("^"),
            Token::Tilde => f.write_str("~"),
            Token::Less => f.write_str("<"),
            Token::Greater => f.write_str(">"),
            Token::ShiftLeft => f.write_str("<<"),
            Token::ShiftRight => f.write_str(">>"),
            Token::Equals => f.write_str("="),
            Token::Here => f.write_str("$"),
            Token::Apostrophe => f.write_str("'"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LexError {
    pub column: usize,
    pub message: String,
}

impl LexError {
    fn new(column: usize, message: impl Into<String>) -> Self {
        Self { column, message: message.into() }
    }
}

/// Split one line of source into tokens, dropping any `;` comment.
pub fn tokenize(line: &str) -> Result<Vec<Token>, LexError> {
    let chars: Vec<char> = line.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        if c == ';' {
            break;
        }
        if c.is_whitespace() {
            i += 1;
            continue;
        }

        // `#` is an optional prefix on constants, but `#INCLUDE` is a
        // directive in its own right.
        if c == '#' {
            if matches!(chars.get(i + 1), Some(n) if n.is_ascii_alphabetic()) {
                let (name, next) = read_word(&chars, i + 1);
                tokens.push(Token::Ident(format!("#{name}")));
                i = next;
            } else {
                i += 1;
            }
            continue;
        }

        if c.is_ascii_digit() {
            let (value, next) = read_number(&chars, i)?;
            tokens.push(Token::Number(value));
            i = next;
            continue;
        }

        if c == '$' {
            // `$` alone is the current address; `$1F` is a hex constant.
            if matches!(chars.get(i + 1), Some(n) if n.is_ascii_hexdigit()) {
                let (digits, next) = read_while(&chars, i + 1, |c| c.is_ascii_hexdigit());
                tokens.push(Token::Number(parse_radix(&digits, 16, i)?));
                i = next;
            } else {
                tokens.push(Token::Here);
                i += 1;
            }
            continue;
        }

        if c.is_ascii_alphabetic() || c == '_' || c == '.' || c == '@' || c == '?' {
            let (name, next) = read_word(&chars, i);
            // `PC` is a synonym for `$`, but only on its own.
            if name == "PC" {
                tokens.push(Token::Here);
            } else {
                tokens.push(Token::Ident(name));
            }
            i = next;
            continue;
        }

        if c == '\'' || c == '"' {
            match read_quoted(&chars, i, c) {
                Some((text, next)) => {
                    tokens.push(if text.chars().count() == 1 {
                        Token::Number(text.chars().next().unwrap() as i64)
                    } else {
                        Token::Text(text)
                    });
                    i = next;
                }
                // An unterminated `'` is the apostrophe of `AF'`.
                None if c == '\'' => {
                    tokens.push(Token::Apostrophe);
                    i += 1;
                }
                None => return Err(LexError::new(i, "unterminated string")),
            }
            continue;
        }

        let (token, width) = match (c, chars.get(i + 1)) {
            ('<', Some('<')) => (Token::ShiftLeft, 2),
            ('>', Some('>')) => (Token::ShiftRight, 2),
            (',', _) => (Token::Comma, 1),
            (':', _) => (Token::Colon, 1),
            ('(', _) | ('[', _) => (Token::LParen, 1),
            (')', _) | (']', _) => (Token::RParen, 1),
            ('+', _) => (Token::Plus, 1),
            ('-', _) => (Token::Minus, 1),
            ('*', _) => (Token::Star, 1),
            ('/', _) => (Token::Slash, 1),
            ('&', _) => (Token::Ampersand, 1),
            ('|', _) => (Token::Pipe, 1),
            ('^', _) => (Token::Caret, 1),
            ('~', _) => (Token::Tilde, 1),
            ('<', _) => (Token::Less, 1),
            ('>', _) => (Token::Greater, 1),
            ('=', _) => (Token::Equals, 1),
            _ => return Err(LexError::new(i, format!("unexpected character '{c}'"))),
        };
        tokens.push(token);
        i += width;
    }

    Ok(tokens)
}

fn read_while(
    chars: &[char],
    start: usize,
    accept: impl Fn(char) -> bool,
) -> (String, usize) {
    let mut end = start;
    while end < chars.len() && accept(chars[end]) {
        end += 1;
    }
    (chars[start..end].iter().collect(), end)
}

fn read_word(chars: &[char], start: usize) -> (String, usize) {
    let (word, end) = read_while(chars, start, |c| {
        c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '@' || c == '?'
    });
    (word.to_ascii_uppercase(), end)
}

/// Read a constant that starts with a digit. The radix may be given by a
/// `0x`/`0b` prefix or by an `H`, `B`, `O`, `Q` or `D` suffix; hex constants
/// beginning with a letter need the usual leading zero (`0FFH`).
fn read_number(chars: &[char], start: usize) -> Result<(i64, usize), LexError> {
    if chars[start] == '0' {
        if let Some(prefix) = chars.get(start + 1) {
            let radix = match prefix.to_ascii_lowercase() {
                'x' => Some(16),
                'b' if is_binary_run(chars, start + 2) => Some(2),
                'o' | 'q' => Some(8),
                _ => None,
            };
            if let Some(radix) = radix {
                let (digits, end) =
                    read_while(chars, start + 2, |c| c.is_digit(radix) || c == '_');
                if !digits.is_empty() {
                    return Ok((parse_radix(&digits, radix, start)?, end));
                }
            }
        }
    }

    let (body, end) = read_while(chars, start, |c| c.is_ascii_alphanumeric() || c == '_');
    let (digits, radix) = match body.chars().last().map(|c| c.to_ascii_lowercase()) {
        Some('h') => (&body[..body.len() - 1], 16),
        Some('b') if body[..body.len() - 1].chars().all(|c| c == '0' || c == '1') => {
            (&body[..body.len() - 1], 2)
        }
        Some('o') | Some('q') => (&body[..body.len() - 1], 8),
        Some('d') => (&body[..body.len() - 1], 10),
        _ => (&body[..], 10),
    };

    Ok((parse_radix(digits, radix, start)?, end))
}

fn is_binary_run(chars: &[char], start: usize) -> bool {
    matches!(chars.get(start), Some('0') | Some('1'))
}

fn parse_radix(digits: &str, radix: u32, column: usize) -> Result<i64, LexError> {
    let cleaned: String = digits.chars().filter(|c| *c != '_').collect();
    i64::from_str_radix(&cleaned, radix).map_err(|_| {
        LexError::new(column, format!("'{digits}' is not a valid base-{radix} number"))
    })
}

/// Read a quoted run, returning `None` when the closing quote is missing.
/// Inside single quotes, `''` is an escaped apostrophe.
fn read_quoted(chars: &[char], start: usize, quote: char) -> Option<(String, usize)> {
    let mut text = String::new();
    let mut i = start + 1;
    while i < chars.len() {
        if chars[i] == quote {
            if chars.get(i + 1) == Some(&quote) {
                text.push(quote);
                i += 2;
                continue;
            }
            return Some((text, i + 1));
        }
        text.push(chars[i]);
        i += 1;
    }
    None
}
