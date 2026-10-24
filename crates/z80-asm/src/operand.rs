//! Operand classification.
//!
//! Operands are kept as token slices rather than being parsed eagerly,
//! because several names are ambiguous until the mnemonic is known: `C` is
//! both a register and the carry condition, and a bare identifier may be a
//! condition code or a label.

use crate::expr::{eval, EvalContext, ExprError};
use crate::lexer::Token;

#[derive(Clone, Copy, Debug)]
pub struct Operand<'a> {
    pub tokens: &'a [Token],
}

/// What a parenthesised operand encloses.
#[derive(Clone, Debug)]
pub enum MemRef<'a> {
    /// `(HL)`, `(BC)`, `(C)`, `(SP)`, `(IX)`, ...
    Register(String),
    /// `(IX+d)` or `(IY-d)`.
    Indexed(String, &'a [Token]),
    /// `(nn)`
    Address(&'a [Token]),
}

impl<'a> Operand<'a> {
    pub fn new(tokens: &'a [Token]) -> Self {
        Self { tokens }
    }

    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    /// The operand when it is a single bare name, such as `A`, `HL` or `NZ`.
    pub fn name(&self) -> Option<&str> {
        match self.tokens {
            [Token::Ident(name)] => Some(name.as_str()),
            _ => None,
        }
    }

    /// True for `AF'`, the only operand that ends in an apostrophe.
    pub fn is_shadow_af(&self) -> bool {
        matches!(self.tokens, [Token::Ident(name), Token::Apostrophe] if name == "AF")
    }

    /// Classify a parenthesised operand, or `None` when the operand is not
    /// wrapped in parentheses at the top level. `(2+3)*4` is an expression,
    /// not a memory reference, so the closing paren must be the last token.
    pub fn memory(&self) -> Option<MemRef<'a>> {
        let [Token::LParen, rest @ .., Token::RParen] = self.tokens else {
            return None;
        };
        if !is_balanced(rest) {
            return None;
        }

        match rest {
            // Only an actual register name makes this a register reference;
            // `(LABEL)` is an address like any other expression.
            [Token::Ident(name)] if is_memory_register(name) => {
                Some(MemRef::Register(name.clone()))
            }
            [Token::Ident(name), displacement @ ..]
                if is_index_register(name)
                    && matches!(displacement.first(), Some(Token::Plus) | Some(Token::Minus)) =>
            {
                Some(MemRef::Indexed(name.clone(), displacement))
            }
            _ => Some(MemRef::Address(rest)),
        }
    }

    /// Evaluate the operand as a constant expression.
    pub fn value(&self, ctx: &EvalContext<'_>) -> Result<i64, ExprError> {
        evaluate(self.tokens, ctx)
    }
}

pub fn evaluate(tokens: &[Token], ctx: &EvalContext<'_>) -> Result<i64, ExprError> {
    let mut pos = 0;
    let value = eval(tokens, &mut pos, ctx)?;
    match tokens.get(pos) {
        None => Ok(value),
        Some(extra) => Err(ExprError::Unexpected(extra.to_string())),
    }
}

/// The registers that can appear inside parentheses as a pointer.
pub fn is_memory_register(name: &str) -> bool {
    matches!(name, "HL" | "BC" | "DE" | "SP" | "C" | "IX" | "IY")
}

pub fn is_index_register(name: &str) -> bool {
    name == "IX" || name == "IY"
}

/// Parentheses are balanced and never close early, so the whole operand is
/// one bracketed group rather than two adjacent ones like `(a)+(b)`.
fn is_balanced(tokens: &[Token]) -> bool {
    let mut depth = 0i32;
    for token in tokens {
        match token {
            Token::LParen => depth += 1,
            Token::RParen => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            _ => {}
        }
    }
    depth == 0
}

/// Split a token run on top-level commas, so that `(IX+1),A` yields two
/// operands but `LD HL,(nn)` is not split inside the parentheses.
pub fn split_operands(tokens: &[Token]) -> Vec<Operand<'_>> {
    if tokens.is_empty() {
        return Vec::new();
    }

    let mut operands = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;

    for (i, token) in tokens.iter().enumerate() {
        match token {
            Token::LParen => depth += 1,
            Token::RParen => depth -= 1,
            Token::Comma if depth == 0 => {
                operands.push(Operand::new(&tokens[start..i]));
                start = i + 1;
            }
            _ => {}
        }
    }
    operands.push(Operand::new(&tokens[start..]));
    operands
}

/// Register code used by the 8-bit instruction encodings.
pub fn reg8_code(name: &str) -> Option<u8> {
    Some(match name {
        "B" => 0,
        "C" => 1,
        "D" => 2,
        "E" => 3,
        "H" => 4,
        "L" => 5,
        "A" => 7,
        _ => return None,
    })
}

/// The undocumented halves of IX and IY, as an index prefix plus the
/// register code they stand in for.
pub fn index_half(name: &str) -> Option<(u8, u8)> {
    Some(match name {
        "IXH" | "HX" | "XH" => (0xDD, 4),
        "IXL" | "LX" | "XL" => (0xDD, 5),
        "IYH" | "HY" | "YH" => (0xFD, 4),
        "IYL" | "LY" | "YL" => (0xFD, 5),
        _ => return None,
    })
}

/// Code for the `BC`/`DE`/`HL`/`SP` group, with `HL` standing in for the
/// active index register.
pub fn reg16_code(name: &str) -> Option<u8> {
    Some(match name {
        "BC" => 0,
        "DE" => 1,
        "HL" => 2,
        "SP" => 3,
        _ => return None,
    })
}

/// Code for the `PUSH`/`POP` group, where `AF` replaces `SP`.
pub fn stack_pair_code(name: &str) -> Option<u8> {
    Some(match name {
        "BC" => 0,
        "DE" => 1,
        "HL" => 2,
        "AF" => 3,
        _ => return None,
    })
}

pub fn condition_code(name: &str) -> Option<u8> {
    Some(match name {
        "NZ" => 0,
        "Z" => 1,
        "NC" => 2,
        "C" => 3,
        "PO" => 4,
        "PE" => 5,
        "P" => 6,
        "M" => 7,
        _ => return None,
    })
}

/// The prefix byte for an index register name, or `None` for `HL`.
pub fn index_prefix(name: &str) -> Option<u8> {
    match name {
        "IX" => Some(0xDD),
        "IY" => Some(0xFD),
        _ => None,
    }
}
