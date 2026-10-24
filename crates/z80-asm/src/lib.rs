//! Z80 assembler, disassembler and object-file formats.
//!
//! The source syntax follows Oshonsoft's Z80 Simulator IDE so that programs
//! written for it assemble here unchanged.

pub mod assemble;
pub mod disasm;
pub mod encode;
pub mod error;
pub mod expr;
pub mod formats;
pub mod lexer;
pub mod listing;
pub mod operand;
pub mod source;

pub use assemble::{assemble, Assembly, Diagnostic, Symbol};
pub use error::{AsmError, AsmResult};
pub use source::{FileLoader, NoIncludes};
