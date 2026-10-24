//! Exhaustive assembler/disassembler agreement check.
//!
//! For every opcode on every prefix page: disassemble the bytes, assemble
//! the text back, and disassemble again. The two texts must match, which
//! means the assembler accepts everything the disassembler emits and encodes
//! it to the same instruction.

use z80_asm::disasm::disassemble_one;
use z80_asm::{assemble, NoIncludes};

const BASE: u16 = 0x2000;
/// Operand bytes that are distinctive enough to notice if they move.
const FILLER: [u8; 4] = [0x12, 0x34, 0x56, 0x78];

fn disassemble(bytes: &[u8]) -> (String, Vec<u8>) {
    let mut memory = [0u8; 16];
    for (index, slot) in memory.iter_mut().enumerate() {
        *slot = FILLER[index % FILLER.len()];
    }
    memory[..bytes.len()].copy_from_slice(bytes);

    let mut read = |address: u16| memory[(address.wrapping_sub(BASE)) as usize % 16];
    let instruction = disassemble_one(&mut read, BASE);
    (instruction.text, instruction.bytes)
}

fn reassemble(text: &str) -> Vec<u8> {
    let program = format!("    ORG {BASE:04X}h\n    {text}\n");
    let result = assemble("roundtrip.asm", &program, &NoIncludes);
    assert!(
        result.succeeded(),
        "`{text}` did not assemble: {}",
        result.errors.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")
    );
    let segments = result.segments();
    assert_eq!(segments.len(), 1, "`{text}` produced {} segments", segments.len());
    segments[0].1.clone()
}

/// Walk every opcode on one page, reporting all mismatches at once.
fn check_page(label: &str, build: impl Fn(u8) -> Vec<u8>) {
    let mut failures = Vec::new();

    for opcode in 0u8..=0xFF {
        let input = build(opcode);
        let (text, consumed) = disassemble(&input);

        let assembled = reassemble(&text);
        let (text_again, _) = disassemble(&assembled);

        if text != text_again {
            failures.push(format!(
                "{label} {opcode:02X}: {consumed:02X?} -> `{text}` -> {assembled:02X?} -> `{text_again}`"
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "{} mismatch(es) on the {label} page:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn unprefixed_page() {
    check_page("base", |opcode| match opcode {
        // The prefix bytes are covered by their own pages.
        0xCB | 0xDD | 0xED | 0xFD => vec![0x00],
        _ => vec![opcode, FILLER[0], FILLER[1], FILLER[2]],
    });
}

#[test]
fn cb_page() {
    check_page("CB", |opcode| vec![0xCB, opcode]);
}

#[test]
fn ed_page() {
    check_page("ED", |opcode| vec![0xED, opcode]);
}

#[test]
fn dd_page() {
    check_page("DD", |opcode| match opcode {
        0xCB | 0xDD | 0xED | 0xFD => vec![0x00],
        _ => vec![0xDD, opcode, FILLER[0], FILLER[1], FILLER[2]],
    });
}

#[test]
fn fd_page() {
    check_page("FD", |opcode| match opcode {
        0xCB | 0xDD | 0xED | 0xFD => vec![0x00],
        _ => vec![0xFD, opcode, FILLER[0], FILLER[1], FILLER[2]],
    });
}

#[test]
fn ddcb_page() {
    check_page("DDCB", |opcode| vec![0xDD, 0xCB, 0x05, opcode]);
}

#[test]
fn fdcb_page() {
    check_page("FDCB", |opcode| vec![0xFD, 0xCB, 0xFB, opcode]);
}
