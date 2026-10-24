//! Listing (`.lst`) and symbol-map output.

use std::fmt::Write;

use crate::assemble::Assembly;

/// How much detail the listing carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ListingOptions {
    /// Leave out the machine-code column, keeping only addresses and source.
    pub without_opcodes: bool,
    /// Append the symbol table.
    pub with_symbols: bool,
}

const BYTES_PER_ROW: usize = 4;

/// Render the assembly listing in the layout Z80 Simulator IDE uses:
/// address, machine code, then the original source line.
pub fn render(assembly: &Assembly, options: ListingOptions) -> String {
    let mut out = String::new();

    for line in &assembly.listing {
        let address = if line.addressless && line.bytes.is_empty() {
            "    ".to_string()
        } else {
            format!("{:04X}", line.address)
        };

        if options.without_opcodes {
            let _ = writeln!(out, "{address}  {}", line.text.trim_end());
            continue;
        }

        let mut chunks = line.bytes.chunks(BYTES_PER_ROW);
        let first = chunks.next().unwrap_or(&[]);
        let _ = writeln!(
            out,
            "{address}  {:<width$}  {}",
            hex_bytes(first),
            line.text.trim_end(),
            width = BYTES_PER_ROW * 3 - 1
        );

        // Instructions never exceed four bytes, but DB and DS runs do, so
        // the rest continue on their own address-stamped rows.
        for (index, chunk) in chunks.enumerate() {
            let offset = line.address.wrapping_add(((index + 1) * BYTES_PER_ROW) as u16);
            let _ = writeln!(out, "{offset:04X}  {}", hex_bytes(chunk));
        }
    }

    if options.with_symbols {
        out.push('\n');
        out.push_str(&render_symbols(assembly));
    }

    if !assembly.errors.is_empty() {
        out.push('\n');
        for error in &assembly.errors {
            let _ = writeln!(out, "{error}");
        }
    }

    out
}

/// The symbol map: every label and `EQU` with its value.
pub fn render_symbols(assembly: &Assembly) -> String {
    let mut out = String::from("Symbol table\n");
    let width = assembly
        .symbols
        .iter()
        .map(|symbol| symbol.name.len())
        .max()
        .unwrap_or(0)
        .max(6);

    for symbol in &assembly.symbols {
        let kind = if symbol.is_address { "label" } else { "equ  " };
        let _ = writeln!(
            out,
            "{:<width$}  {:04X}  {kind}  {}({})",
            symbol.name,
            symbol.value as u16,
            symbol.file,
            symbol.line,
            width = width
        );
    }

    out
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}
