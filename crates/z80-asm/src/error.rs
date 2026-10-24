//! Assembly errors, before they are attached to a source line.

use crate::expr::ExprError;

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AsmError {
    #[error("{0}")]
    Syntax(String),

    #[error("{0}")]
    Expr(#[from] ExprError),

    #[error("unknown instruction or directive '{0}'")]
    UnknownMnemonic(String),

    #[error("{mnemonic} does not accept these operands")]
    BadOperands { mnemonic: String },

    #[error("value {value} does not fit in {bits} bits")]
    Range { value: i64, bits: u32 },

    #[error("relative jump of {distance} bytes is out of range (-128..+127)")]
    RelativeRange { distance: i64 },

    #[error("'{0}' is already defined")]
    DuplicateSymbol(String),

    #[error("{0}")]
    Io(String),
}

pub type AsmResult<T> = Result<T, AsmError>;

/// Accept any value that fits in a byte read as either signed or unsigned.
pub fn to_byte(value: i64) -> AsmResult<u8> {
    if (-128..=255).contains(&value) {
        Ok(value as u8)
    } else {
        Err(AsmError::Range { value, bits: 8 })
    }
}

/// Accept any value that fits in a word read as either signed or unsigned.
pub fn to_word(value: i64) -> AsmResult<u16> {
    if (-32768..=65535).contains(&value) {
        Ok(value as u16)
    } else {
        Err(AsmError::Range { value, bits: 16 })
    }
}

/// Index-register displacements are always signed.
pub fn to_displacement(value: i64) -> AsmResult<u8> {
    if (-128..=127).contains(&value) {
        Ok(value as u8)
    } else {
        Err(AsmError::Range { value, bits: 8 })
    }
}
