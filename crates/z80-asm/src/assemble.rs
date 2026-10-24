//! The two-pass assembler.
//!
//! Pass one records where every label lands; pass two evaluates expressions
//! and emits code. Instruction sizes never depend on the values of the
//! symbols involved, so one pass is enough to fix every address.

use std::collections::BTreeMap;

use crate::encode::encode;
use crate::error::{to_byte, to_word, AsmError};
use crate::expr::{EvalContext, SymbolTable};
use crate::lexer::{tokenize, Token};
use crate::operand::split_operands;
use crate::source::{flatten, SourceLine, SourceLoader};

/// A symbol and where it was defined.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Symbol {
    pub name: String,
    pub value: i64,
    pub file: String,
    pub line: usize,
    /// True for labels, false for `EQU` constants.
    pub is_address: bool,
}

/// One line of the assembly listing.
#[derive(Clone, Debug)]
pub struct ListingLine {
    pub address: u16,
    pub bytes: Vec<u8>,
    pub file: String,
    pub number: usize,
    pub text: String,
    /// True when the line produced no address (a comment, `EQU` or `END`).
    pub addressless: bool,
}

#[derive(Clone, Debug)]
pub struct Diagnostic {
    pub file: String,
    pub line: usize,
    pub message: String,
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}({}): {}", self.file, self.line, self.message)
    }
}

/// The result of assembling a program.
pub struct Assembly {
    /// The full 64K address space. Only the bytes flagged in `written` were
    /// produced by this program.
    pub image: Box<[u8; 0x10000]>,
    pub written: Box<[bool; 0x10000]>,
    pub listing: Vec<ListingLine>,
    pub symbols: Vec<Symbol>,
    pub errors: Vec<Diagnostic>,
    /// The address of the first `ORG`, which is where execution starts.
    pub origin: u16,
}

impl Assembly {
    pub fn succeeded(&self) -> bool {
        self.errors.is_empty()
    }

    /// The contiguous runs of bytes the program actually defined.
    pub fn segments(&self) -> Vec<(u16, Vec<u8>)> {
        let mut segments = Vec::new();
        let mut address = 0usize;

        while address < 0x10000 {
            if !self.written[address] {
                address += 1;
                continue;
            }
            let start = address;
            while address < 0x10000 && self.written[address] {
                address += 1;
            }
            segments.push((start as u16, self.image[start..address].to_vec()));
        }

        segments
    }

    /// Lowest and highest addresses defined, or `None` for an empty program.
    pub fn extent(&self) -> Option<(u16, u16)> {
        let first = self.written.iter().position(|w| *w)?;
        let last = self.written.iter().rposition(|w| *w)?;
        Some((first as u16, last as u16))
    }

    pub fn symbol(&self, name: &str) -> Option<&Symbol> {
        let upper = name.to_ascii_uppercase();
        self.symbols.iter().find(|symbol| symbol.name == upper)
    }
}

/// Assemble `text`, following any `INCLUDE` directives through `loader`.
pub fn assemble(name: &str, text: &str, loader: &dyn SourceLoader) -> Assembly {
    let (lines, include_errors) = flatten(name, text, loader);

    let mut errors: Vec<Diagnostic> = include_errors
        .into_iter()
        .map(|(line, message)| Diagnostic {
            file: line.file,
            line: line.number,
            message,
        })
        .collect();

    // Pass one: place the labels. Expression errors here are suppressed,
    // because forward references are expected and will be resolved in pass
    // two; only pass two reports diagnostics.
    let mut symbols = BTreeMap::new();
    let origin = run_pass(&lines, &mut symbols, &mut Vec::new(), None);

    // Pass two: emit code with every symbol known.
    let mut image = Box::new([0u8; 0x10000]);
    let mut written = Box::new([false; 0x10000]);
    let mut listing = Vec::new();
    let mut output = Output {
        image: &mut image,
        written: &mut written,
        listing: &mut listing,
    };
    run_pass(&lines, &mut symbols.clone(), &mut errors, Some(&mut output));

    let mut symbols: Vec<Symbol> = symbols.into_values().collect();
    symbols.sort_by(|a, b| a.name.cmp(&b.name));

    Assembly { image, written, listing, symbols, errors, origin }
}

struct Output<'a> {
    image: &'a mut [u8; 0x10000],
    written: &'a mut [bool; 0x10000],
    listing: &'a mut Vec<ListingLine>,
}

/// A symbol table view that pass two hands to the expression evaluator.
struct Symbols<'a>(&'a BTreeMap<String, Symbol>);

impl SymbolTable for Symbols<'_> {
    fn lookup(&self, name: &str) -> Option<i64> {
        self.0.get(name).map(|symbol| symbol.value)
    }
}

/// Run one pass over the flattened source. Returns the first `ORG` address.
fn run_pass(
    lines: &[SourceLine],
    symbols: &mut BTreeMap<String, Symbol>,
    errors: &mut Vec<Diagnostic>,
    mut output: Option<&mut Output<'_>>,
) -> u16 {
    let emitting = output.is_some();
    let mut address: u16 = 0;
    let mut origin: Option<u16> = None;

    for line in lines {
        let mut report = |message: String| {
            if emitting {
                errors.push(Diagnostic {
                    file: line.file.clone(),
                    line: line.number,
                    message,
                });
            }
        };

        let tokens = match tokenize(&line.text) {
            Ok(tokens) => tokens,
            Err(error) => {
                report(error.message);
                continue;
            }
        };

        let parsed = match parse_line(&tokens, &line.text) {
            Ok(parsed) => parsed,
            Err(message) => {
                report(message);
                continue;
            }
        };

        // A label takes the current address unless the line is an `EQU`,
        // which gives it a value of its own instead.
        let is_equate = matches!(parsed.body, Body::Equate(_));
        if let Some(label) = &parsed.label {
            if !is_equate {
                define(symbols, label, address as i64, line, true, &mut report);
            }
        }

        let ctx_symbols = Symbols(symbols);
        let ctx = EvalContext {
            pc: address,
            symbols: &ctx_symbols,
            allow_unknown: !emitting,
        };

        let mut bytes: Vec<u8> = Vec::new();
        let mut addressless = false;

        match &parsed.body {
            Body::Empty => addressless = true,

            Body::Equate(tokens) => {
                addressless = true;
                let Some(label) = &parsed.label else {
                    report("EQU needs a label".to_string());
                    continue;
                };
                match crate::operand::evaluate(tokens, &ctx) {
                    Ok(value) => {
                        // Redefinition is how pass two refreshes the table,
                        // so only a conflicting value is an error.
                        let symbols_ref = &mut *symbols;
                        match symbols_ref.get(label.as_str()) {
                            Some(existing) if existing.value != value && emitting => {
                                report(AsmError::DuplicateSymbol(label.clone()).to_string());
                            }
                            _ => {
                                define_raw(symbols_ref, label, value, line, false);
                            }
                        }
                    }
                    Err(error) => report(error.to_string()),
                }
            }

            Body::Org(tokens) => {
                addressless = true;
                match crate::operand::evaluate(tokens, &ctx).map_err(AsmError::from).and_then(to_word) {
                    Ok(value) => {
                        address = value;
                        if origin.is_none() {
                            origin = Some(value);
                        }
                        // A label on an ORG line names the new address.
                        if let Some(label) = &parsed.label {
                            define_raw(symbols, label, value as i64, line, true);
                        }
                    }
                    Err(error) => report(error.to_string()),
                }
            }

            Body::End => addressless = true,

            Body::DefineBytes(operands) => {
                for operand in operands {
                    match operand.as_slice() {
                        [Token::Text(text)] => bytes.extend(text.bytes()),
                        tokens => match crate::operand::evaluate(tokens, &ctx)
                            .map_err(AsmError::from)
                            .and_then(to_byte)
                        {
                            Ok(value) => bytes.push(value),
                            Err(error) => report(error.to_string()),
                        },
                    }
                }
            }

            Body::DefineWords(operands) => {
                for operand in operands {
                    match crate::operand::evaluate(operand, &ctx)
                        .map_err(AsmError::from)
                        .and_then(to_word)
                    {
                        Ok(value) => bytes.extend(value.to_le_bytes()),
                        Err(error) => report(error.to_string()),
                    }
                }
            }

            Body::Reserve(tokens) => match crate::operand::evaluate(tokens, &ctx) {
                Ok(count) if (0..=0x10000).contains(&count) => {
                    bytes.resize(count as usize, 0);
                }
                Ok(count) => report(format!("cannot reserve {count} bytes")),
                Err(error) => report(error.to_string()),
            },

            Body::Instruction { mnemonic, operands } => {
                let operand_refs = split_operands(operands);
                match encode(mnemonic, &operand_refs, &ctx) {
                    Ok(encoded) => bytes = encoded,
                    Err(error) => report(error.to_string()),
                }
            }
        }

        if let Some(output) = output.as_deref_mut() {
            output.listing.push(ListingLine {
                address,
                bytes: bytes.clone(),
                file: line.file.clone(),
                number: line.number,
                text: line.text.clone(),
                addressless,
            });

            for (offset, byte) in bytes.iter().enumerate() {
                let target = address.wrapping_add(offset as u16) as usize;
                if output.written[target] && emitting {
                    // Overlapping ORGs are legal but worth flagging.
                    errors.push(Diagnostic {
                        file: line.file.clone(),
                        line: line.number,
                        message: format!("address {target:04X}h is written twice"),
                    });
                }
                output.image[target] = *byte;
                output.written[target] = true;
            }
        }

        address = address.wrapping_add(bytes.len() as u16);
    }

    origin.unwrap_or(0)
}

fn define(
    symbols: &mut BTreeMap<String, Symbol>,
    name: &str,
    value: i64,
    line: &SourceLine,
    is_address: bool,
    report: &mut impl FnMut(String),
) {
    if let Some(existing) = symbols.get(name) {
        if existing.value != value && existing.line != line.number {
            report(AsmError::DuplicateSymbol(name.to_string()).to_string());
            return;
        }
    }
    define_raw(symbols, name, value, line, is_address);
}

fn define_raw(
    symbols: &mut BTreeMap<String, Symbol>,
    name: &str,
    value: i64,
    line: &SourceLine,
    is_address: bool,
) {
    symbols.insert(
        name.to_string(),
        Symbol {
            name: name.to_string(),
            value,
            file: line.file.clone(),
            line: line.number,
            is_address,
        },
    );
}

/// What a source line asks the assembler to do.
enum Body {
    Empty,
    Org(Vec<Token>),
    Equate(Vec<Token>),
    End,
    DefineBytes(Vec<Vec<Token>>),
    DefineWords(Vec<Vec<Token>>),
    Reserve(Vec<Token>),
    Instruction { mnemonic: String, operands: Vec<Token> },
}

struct ParsedLine {
    label: Option<String>,
    body: Body,
}

/// Split a line into its optional label and its body.
fn parse_line(tokens: &[Token], raw: &str) -> Result<ParsedLine, String> {
    if tokens.is_empty() {
        return Ok(ParsedLine { label: None, body: Body::Empty });
    }

    let mut rest = tokens;
    let mut label = None;

    if let Token::Ident(name) = &tokens[0] {
        let starts_at_margin = !raw.starts_with(char::is_whitespace);
        if tokens.get(1) == Some(&Token::Colon) {
            label = Some(name.clone());
            rest = &tokens[2..];
        } else if starts_at_margin && !is_reserved(name) {
            label = Some(name.clone());
            rest = &tokens[1..];
        }
    }

    if rest.is_empty() {
        return Ok(ParsedLine { label, body: Body::Empty });
    }

    // `LABEL = value` is a synonym for `LABEL EQU value`.
    if rest[0] == Token::Equals {
        return Ok(ParsedLine { label, body: Body::Equate(rest[1..].to_vec()) });
    }

    let Token::Ident(word) = &rest[0] else {
        return Err(format!("expected an instruction, found '{}'", rest[0]));
    };

    // A leading dot on a directive is optional: `.ORG` and `ORG` are the same.
    let directive = word.trim_start_matches('.');
    let arguments = &rest[1..];

    let body = match directive {
        "EQU" => Body::Equate(arguments.to_vec()),
        "ORG" => Body::Org(arguments.to_vec()),
        "END" => Body::End,
        "DB" | "DEFB" | "BYTE" => Body::DefineBytes(split_list(arguments)),
        "DW" | "DEFW" | "WORD" => Body::DefineWords(split_list(arguments)),
        "DS" | "DEFS" | "BLOCK" => Body::Reserve(arguments.to_vec()),
        // Includes are expanded before assembly; the line itself is inert.
        "INCLUDE" | "#INCLUDE" => Body::Empty,
        _ => Body::Instruction {
            mnemonic: word.clone(),
            operands: arguments.to_vec(),
        },
    };

    Ok(ParsedLine { label, body })
}

/// Words that can never be a label, because a line starting with one is an
/// instruction or directive even at the left margin.
fn is_reserved(word: &str) -> bool {
    const DIRECTIVES: &[&str] = &[
        "ORG", "END", "EQU", "DB", "DEFB", "BYTE", "DW", "DEFW", "WORD", "DS", "DEFS",
        "BLOCK", "INCLUDE", "#INCLUDE",
    ];
    let bare = word.trim_start_matches('.');
    DIRECTIVES.contains(&bare) || crate::encode::is_mnemonic(bare)
}

/// Split a directive's comma-separated arguments.
fn split_list(tokens: &[Token]) -> Vec<Vec<Token>> {
    if tokens.is_empty() {
        return Vec::new();
    }
    split_operands(tokens)
        .into_iter()
        .map(|operand| operand.tokens.to_vec())
        .collect()
}
