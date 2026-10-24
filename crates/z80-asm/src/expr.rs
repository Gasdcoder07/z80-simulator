//! Constant-expression parsing and evaluation.
//!
//! Supports the operator set documented for Z80 Simulator IDE: arithmetic,
//! bitwise and shift operators, parentheses, `$`/`PC` for the current
//! address, the `<` and `>` byte-select prefixes and the `loww`/`highw`
//! word-select prefixes.

use crate::lexer::Token;

/// Somewhere to look up label and `EQU` values.
pub trait SymbolTable {
    fn lookup(&self, name: &str) -> Option<i64>;
}

impl<F: Fn(&str) -> Option<i64>> SymbolTable for F {
    fn lookup(&self, name: &str) -> Option<i64> {
        self(name)
    }
}

pub struct EvalContext<'a> {
    /// Value of `$` — the address of the instruction being assembled.
    pub pc: u16,
    pub symbols: &'a dyn SymbolTable,
    /// During the first pass, forward references are not yet known; they
    /// evaluate to zero so that instruction sizes can still be determined.
    pub allow_unknown: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExprError {
    UnknownSymbol(String),
    DivisionByZero,
    UnexpectedEnd,
    Unexpected(String),
    MissingParen,
}

impl std::fmt::Display for ExprError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExprError::UnknownSymbol(name) => write!(f, "undefined symbol '{name}'"),
            ExprError::DivisionByZero => f.write_str("division by zero"),
            ExprError::UnexpectedEnd => f.write_str("unexpected end of expression"),
            ExprError::Unexpected(token) => write!(f, "unexpected '{token}' in expression"),
            ExprError::MissingParen => f.write_str("missing ')'"),
        }
    }
}

impl std::error::Error for ExprError {}

/// Parse and evaluate an expression starting at `pos`, leaving `pos` on the
/// first token that is not part of it.
pub fn eval(
    tokens: &[Token],
    pos: &mut usize,
    ctx: &EvalContext<'_>,
) -> Result<i64, ExprError> {
    parse_or(tokens, pos, ctx)
}

/// True when the token could begin an expression. Used to tell `RET` from
/// `RET cc` and similar.
pub fn starts_expression(token: &Token) -> bool {
    matches!(
        token,
        Token::Number(_)
            | Token::Text(_)
            | Token::Ident(_)
            | Token::Here
            | Token::LParen
            | Token::Plus
            | Token::Minus
            | Token::Tilde
            | Token::Less
            | Token::Greater
    )
}

type BinaryOp = fn(i64, i64) -> Result<i64, ExprError>;

/// One precedence level: parse a tighter-binding operand, then fold in as
/// many `operator operand` pairs as `select` recognises.
fn binary_level(
    tokens: &[Token],
    pos: &mut usize,
    ctx: &EvalContext<'_>,
    next: fn(&[Token], &mut usize, &EvalContext<'_>) -> Result<i64, ExprError>,
    select: fn(&Token) -> Option<BinaryOp>,
) -> Result<i64, ExprError> {
    let mut left = next(tokens, pos, ctx)?;
    while let Some(op) = tokens.get(*pos).and_then(select) {
        *pos += 1;
        let right = next(tokens, pos, ctx)?;
        left = op(left, right)?;
    }
    Ok(left)
}

/// The operator name, for the word forms (`AND`, `SHL`, `MOD`, ...).
fn word(token: &Token) -> &str {
    match token {
        Token::Ident(name) => name.as_str(),
        _ => "",
    }
}

fn parse_or(tokens: &[Token], pos: &mut usize, ctx: &EvalContext<'_>) -> Result<i64, ExprError> {
    binary_level(tokens, pos, ctx, parse_xor, |t| {
        match (t, word(t)) {
            (Token::Pipe, _) | (_, "OR") => Some(|a, b| Ok(a | b)),
            _ => None,
        }
    })
}

fn parse_xor(tokens: &[Token], pos: &mut usize, ctx: &EvalContext<'_>) -> Result<i64, ExprError> {
    binary_level(tokens, pos, ctx, parse_and, |t| {
        match (t, word(t)) {
            (Token::Caret, _) | (_, "XOR") => Some(|a, b| Ok(a ^ b)),
            _ => None,
        }
    })
}

fn parse_and(tokens: &[Token], pos: &mut usize, ctx: &EvalContext<'_>) -> Result<i64, ExprError> {
    binary_level(tokens, pos, ctx, parse_shift, |t| {
        match (t, word(t)) {
            (Token::Ampersand, _) | (_, "AND") => Some(|a, b| Ok(a & b)),
            _ => None,
        }
    })
}

fn parse_shift(tokens: &[Token], pos: &mut usize, ctx: &EvalContext<'_>) -> Result<i64, ExprError> {
    binary_level(tokens, pos, ctx, parse_additive, |t| {
        match (t, word(t)) {
            (Token::ShiftLeft, _) | (_, "SHL") => Some(|a, b| Ok(a << (b & 63))),
            (Token::ShiftRight, _) | (_, "SHR") => Some(|a, b| Ok(a >> (b & 63))),
            _ => None,
        }
    })
}

fn parse_additive(
    tokens: &[Token],
    pos: &mut usize,
    ctx: &EvalContext<'_>,
) -> Result<i64, ExprError> {
    binary_level(tokens, pos, ctx, parse_multiplicative, |t| match t {
        Token::Plus => Some(|a, b| Ok(a + b)),
        Token::Minus => Some(|a, b| Ok(a - b)),
        _ => None,
    })
}

fn parse_multiplicative(
    tokens: &[Token],
    pos: &mut usize,
    ctx: &EvalContext<'_>,
) -> Result<i64, ExprError> {
    binary_level(tokens, pos, ctx, parse_unary, |t| {
        match (t, word(t)) {
            (Token::Star, _) => Some(|a, b| Ok(a * b)),
            (Token::Slash, _) => Some(|a, b| {
                if b == 0 {
                    Err(ExprError::DivisionByZero)
                } else {
                    Ok(a / b)
                }
            }),
            (_, "MOD") => Some(|a, b| {
                if b == 0 {
                    Err(ExprError::DivisionByZero)
                } else {
                    Ok(a % b)
                }
            }),
            _ => None,
        }
    })
}

fn parse_unary(
    tokens: &[Token],
    pos: &mut usize,
    ctx: &EvalContext<'_>,
) -> Result<i64, ExprError> {
    let token = tokens.get(*pos).ok_or(ExprError::UnexpectedEnd)?;

    let apply: fn(i64) -> i64 = match token {
        Token::Minus => |v| -v,
        Token::Plus => |v| v,
        Token::Tilde => |v| !v,
        // `<` and `>` select the low and high byte of a 16-bit value.
        Token::Less => |v| v & 0xFF,
        Token::Greater => |v| (v >> 8) & 0xFF,
        Token::Ident(name) => match name.as_str() {
            "NOT" => |v| !v,
            "LOW" => |v| v & 0xFF,
            "HIGH" => |v| (v >> 8) & 0xFF,
            // The word selects pick a 16-bit half out of a 32-bit value.
            "LOWW" => |v| v & 0xFFFF,
            "HIGHW" => |v| (v >> 16) & 0xFFFF,
            _ => return parse_primary(tokens, pos, ctx),
        },
        _ => return parse_primary(tokens, pos, ctx),
    };

    *pos += 1;
    Ok(apply(parse_unary(tokens, pos, ctx)?))
}

fn parse_primary(
    tokens: &[Token],
    pos: &mut usize,
    ctx: &EvalContext<'_>,
) -> Result<i64, ExprError> {
    let token = tokens.get(*pos).ok_or(ExprError::UnexpectedEnd)?.clone();
    *pos += 1;

    match token {
        Token::Number(value) => Ok(value),
        Token::Here => Ok(ctx.pc as i64),
        // A two-character literal used in an expression is a 16-bit word,
        // high byte first, as in `LD HL,'AB'`.
        Token::Text(text) => {
            let bytes: Vec<u8> = text.bytes().collect();
            match bytes.len() {
                1 => Ok(bytes[0] as i64),
                2 => Ok(((bytes[0] as i64) << 8) | bytes[1] as i64),
                _ => Err(ExprError::Unexpected(format!("\"{text}\""))),
            }
        }
        Token::Ident(name) => match ctx.symbols.lookup(&name) {
            Some(value) => Ok(value),
            None if ctx.allow_unknown => Ok(0),
            None => Err(ExprError::UnknownSymbol(name)),
        },
        Token::LParen => {
            let value = eval(tokens, pos, ctx)?;
            if tokens.get(*pos) != Some(&Token::RParen) {
                return Err(ExprError::MissingParen);
            }
            *pos += 1;
            Ok(value)
        }
        other => Err(ExprError::Unexpected(other.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;

    fn value(source: &str) -> i64 {
        let tokens = tokenize(source).unwrap();
        let symbols = |name: &str| if name == "START" { Some(0x1234) } else { None };
        let ctx = EvalContext { pc: 0x8000, symbols: &symbols, allow_unknown: false };
        let mut pos = 0;
        let result = eval(&tokens, &mut pos, &ctx).unwrap();
        assert_eq!(pos, tokens.len(), "expression did not consume all tokens");
        result
    }

    #[test]
    fn radix_forms_agree() {
        assert_eq!(value("255"), 255);
        assert_eq!(value("0FFh"), 255);
        assert_eq!(value("$FF"), 255);
        assert_eq!(value("11111111b"), 255);
        assert_eq!(value("0xFF"), 255);
        assert_eq!(value("#255"), 255);
    }

    #[test]
    fn precedence_follows_arithmetic_convention() {
        assert_eq!(value("2+3*4"), 14);
        assert_eq!(value("(2+3)*4"), 20);
        assert_eq!(value("1 shl 3 + 1"), 16);
        assert_eq!(value("6 mod 4"), 2);
    }

    #[test]
    fn byte_and_word_selects() {
        assert_eq!(value(">START"), 0x12);
        assert_eq!(value("<START"), 0x34);
        assert_eq!(value("high START"), 0x12);
        assert_eq!(value("loww 12345678h"), 0x5678);
        assert_eq!(value("highw 12345678h"), 0x1234);
    }

    #[test]
    fn dollar_is_the_current_address() {
        assert_eq!(value("$"), 0x8000);
        assert_eq!(value("$-2"), 0x7FFE);
    }

    #[test]
    fn character_literals() {
        assert_eq!(value("'A'"), 65);
        assert_eq!(value("'A'+1"), 66);
    }
}
