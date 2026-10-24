//! Encoding checks against values taken from the Zilog Z80 CPU User Manual.

use z80_asm::{assemble, NoIncludes};

/// Assemble one line at address 0x1000 and return the bytes it produced.
fn bytes_of(source: &str) -> Vec<u8> {
    let program = format!("    ORG 1000h\n    {source}\n");
    let result = assemble("test.asm", &program, &NoIncludes);
    assert!(
        result.succeeded(),
        "`{source}` failed to assemble: {}",
        result
            .errors
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; ")
    );
    let segments = result.segments();
    assert_eq!(segments.len(), 1, "`{source}` produced {} segments", segments.len());
    assert_eq!(segments[0].0, 0x1000);
    segments[0].1.clone()
}

fn check(cases: &[(&str, &[u8])]) {
    let mut failures = Vec::new();
    for (source, expected) in cases {
        let actual = bytes_of(source);
        if actual != *expected {
            failures.push(format!(
                "{source:<24} expected {expected:02X?}, got {actual:02X?}"
            ));
        }
    }
    assert!(failures.is_empty(), "{} wrong encoding(s):\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn eight_bit_loads() {
    check(&[
        ("LD A,B", &[0x78]),
        ("LD H,(HL)", &[0x66]),
        ("LD (HL),E", &[0x73]),
        ("LD B,25h", &[0x06, 0x25]),
        ("LD (HL),28h", &[0x36, 0x28]),
        ("LD A,(BC)", &[0x0A]),
        ("LD A,(DE)", &[0x1A]),
        ("LD A,(8000h)", &[0x3A, 0x00, 0x80]),
        ("LD (BC),A", &[0x02]),
        ("LD (DE),A", &[0x12]),
        ("LD (8000h),A", &[0x32, 0x00, 0x80]),
        ("LD A,I", &[0xED, 0x57]),
        ("LD A,R", &[0xED, 0x5F]),
        ("LD I,A", &[0xED, 0x47]),
        ("LD R,A", &[0xED, 0x4F]),
        ("LD C,(IX+5)", &[0xDD, 0x4E, 0x05]),
        ("LD B,(IY-3)", &[0xFD, 0x46, 0xFD]),
        ("LD (IX+2),D", &[0xDD, 0x72, 0x02]),
        ("LD (IY+0),A", &[0xFD, 0x77, 0x00]),
        ("LD (IX+4),0A5h", &[0xDD, 0x36, 0x04, 0xA5]),
        // Undocumented index halves.
        ("LD IXH,B", &[0xDD, 0x60]),
        ("LD A,IYL", &[0xFD, 0x7D]),
        ("LD IXL,12h", &[0xDD, 0x2E, 0x12]),
    ]);
}

#[test]
fn sixteen_bit_loads_and_stack() {
    check(&[
        ("LD HL,1234h", &[0x21, 0x34, 0x12]),
        ("LD SP,0FFFFh", &[0x31, 0xFF, 0xFF]),
        ("LD IX,2000h", &[0xDD, 0x21, 0x00, 0x20]),
        ("LD IY,2000h", &[0xFD, 0x21, 0x00, 0x20]),
        ("LD HL,(4545h)", &[0x2A, 0x45, 0x45]),
        ("LD BC,(4545h)", &[0xED, 0x4B, 0x45, 0x45]),
        ("LD SP,(4545h)", &[0xED, 0x7B, 0x45, 0x45]),
        ("LD IX,(4545h)", &[0xDD, 0x2A, 0x45, 0x45]),
        ("LD (4545h),HL", &[0x22, 0x45, 0x45]),
        ("LD (4545h),DE", &[0xED, 0x53, 0x45, 0x45]),
        ("LD (4545h),IY", &[0xFD, 0x22, 0x45, 0x45]),
        ("LD SP,HL", &[0xF9]),
        ("LD SP,IX", &[0xDD, 0xF9]),
        ("PUSH AF", &[0xF5]),
        ("PUSH IX", &[0xDD, 0xE5]),
        ("POP BC", &[0xC1]),
        ("POP IY", &[0xFD, 0xE1]),
        ("EX DE,HL", &[0xEB]),
        ("EX AF,AF'", &[0x08]),
        ("EXX", &[0xD9]),
        ("EX (SP),HL", &[0xE3]),
        ("EX (SP),IX", &[0xDD, 0xE3]),
    ]);
}

#[test]
fn arithmetic_and_logic() {
    check(&[
        ("ADD A,C", &[0x81]),
        ("ADD A,(HL)", &[0x86]),
        ("ADD A,(IX+3)", &[0xDD, 0x86, 0x03]),
        ("ADD A,20h", &[0xC6, 0x20]),
        ("ADC A,B", &[0x88]),
        ("SUB D", &[0x92]),
        ("SUB A,D", &[0x92]),
        ("SBC A,(HL)", &[0x9E]),
        ("AND 7Fh", &[0xE6, 0x7F]),
        ("OR L", &[0xB5]),
        ("XOR (IY-1)", &[0xFD, 0xAE, 0xFF]),
        ("CP 0FEh", &[0xFE, 0xFE]),
        ("INC A", &[0x3C]),
        ("INC (HL)", &[0x34]),
        ("INC (IX+1)", &[0xDD, 0x34, 0x01]),
        ("DEC E", &[0x1D]),
        ("ADD HL,BC", &[0x09]),
        ("ADD HL,HL", &[0x29]),
        ("ADC HL,DE", &[0xED, 0x5A]),
        ("SBC HL,SP", &[0xED, 0x72]),
        ("ADD IX,BC", &[0xDD, 0x09]),
        ("ADD IX,IX", &[0xDD, 0x29]),
        ("ADD IY,SP", &[0xFD, 0x39]),
        ("INC BC", &[0x03]),
        ("INC IX", &[0xDD, 0x23]),
        ("DEC SP", &[0x3B]),
        ("DEC IY", &[0xFD, 0x2B]),
        ("DAA", &[0x27]),
        ("CPL", &[0x2F]),
        ("NEG", &[0xED, 0x44]),
        ("CCF", &[0x3F]),
        ("SCF", &[0x37]),
    ]);
}

#[test]
fn rotates_shifts_and_bits() {
    check(&[
        ("RLCA", &[0x07]),
        ("RLA", &[0x17]),
        ("RRCA", &[0x0F]),
        ("RRA", &[0x1F]),
        ("RLC B", &[0xCB, 0x00]),
        ("RLC (HL)", &[0xCB, 0x06]),
        ("RLC (IX+2)", &[0xDD, 0xCB, 0x02, 0x06]),
        ("RL C", &[0xCB, 0x11]),
        ("RR D", &[0xCB, 0x1A]),
        ("SLA E", &[0xCB, 0x23]),
        ("SRA H", &[0xCB, 0x2C]),
        ("SLL L", &[0xCB, 0x35]),
        ("SRL A", &[0xCB, 0x3F]),
        ("RLD", &[0xED, 0x6F]),
        ("RRD", &[0xED, 0x67]),
        ("BIT 0,B", &[0xCB, 0x40]),
        ("BIT 7,(HL)", &[0xCB, 0x7E]),
        ("BIT 4,(IY+3)", &[0xFD, 0xCB, 0x03, 0x66]),
        ("SET 2,A", &[0xCB, 0xD7]),
        ("RES 6,(HL)", &[0xCB, 0xB6]),
        ("SET 0,(IX+1)", &[0xDD, 0xCB, 0x01, 0xC6]),
        // Undocumented: store the result in a register as well.
        ("SET 0,(IX+1),B", &[0xDD, 0xCB, 0x01, 0xC0]),
        ("RLC (IY-1),C", &[0xFD, 0xCB, 0xFF, 0x01]),
    ]);
}

#[test]
fn jumps_calls_and_returns() {
    check(&[
        ("JP 8000h", &[0xC3, 0x00, 0x80]),
        ("JP NZ,8000h", &[0xC2, 0x00, 0x80]),
        ("JP M,8000h", &[0xFA, 0x00, 0x80]),
        ("JP (HL)", &[0xE9]),
        ("JP (IX)", &[0xDD, 0xE9]),
        // Relative jumps are measured from the byte after the instruction.
        ("JR 1002h", &[0x18, 0x00]),
        ("JR 1000h", &[0x18, 0xFE]),
        ("JR NZ,1010h", &[0x20, 0x0E]),
        ("JR C,0FF0h", &[0x38, 0xEE]),
        ("DJNZ 1000h", &[0x10, 0xFE]),
        ("CALL 8000h", &[0xCD, 0x00, 0x80]),
        ("CALL PE,8000h", &[0xEC, 0x00, 0x80]),
        ("RET", &[0xC9]),
        ("RET NC", &[0xD0]),
        ("RETI", &[0xED, 0x4D]),
        ("RETN", &[0xED, 0x45]),
        ("RST 0", &[0xC7]),
        ("RST 28h", &[0xEF]),
        ("RST 38h", &[0xFF]),
    ]);
}

#[test]
fn ports_interrupts_and_block_operations() {
    check(&[
        ("IN A,(7Eh)", &[0xDB, 0x7E]),
        ("IN B,(C)", &[0xED, 0x40]),
        ("IN F,(C)", &[0xED, 0x70]),
        ("OUT (7Eh),A", &[0xD3, 0x7E]),
        ("OUT (C),D", &[0xED, 0x51]),
        ("OUT (C),0", &[0xED, 0x71]),
        ("DI", &[0xF3]),
        ("EI", &[0xFB]),
        ("IM 0", &[0xED, 0x46]),
        ("IM 1", &[0xED, 0x56]),
        ("IM 2", &[0xED, 0x5E]),
        ("NOP", &[0x00]),
        ("HALT", &[0x76]),
        ("LDI", &[0xED, 0xA0]),
        ("LDIR", &[0xED, 0xB0]),
        ("LDD", &[0xED, 0xA8]),
        ("LDDR", &[0xED, 0xB8]),
        ("CPI", &[0xED, 0xA1]),
        ("CPIR", &[0xED, 0xB1]),
        ("CPD", &[0xED, 0xA9]),
        ("CPDR", &[0xED, 0xB9]),
        ("INI", &[0xED, 0xA2]),
        ("INIR", &[0xED, 0xB2]),
        ("IND", &[0xED, 0xAA]),
        ("INDR", &[0xED, 0xBA]),
        ("OUTI", &[0xED, 0xA3]),
        ("OTIR", &[0xED, 0xB3]),
        ("OUTD", &[0xED, 0xAB]),
        ("OTDR", &[0xED, 0xBB]),
    ]);
}
