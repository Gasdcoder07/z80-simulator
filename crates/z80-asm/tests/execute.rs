//! End-to-end check: a program written in the assembler's own syntax is
//! assembled and then run on the CPU core, so the two halves of the project
//! are validated against each other rather than only against themselves.

use z80_asm::{assemble, NoIncludes};
use z80_core::{Bus, Cpu};

struct Memory {
    bytes: Box<[u8; 0x10000]>,
    ports: Vec<(u16, u8)>,
}

impl Bus for Memory {
    fn read(&mut self, addr: u16) -> u8 {
        self.bytes[addr as usize]
    }
    fn write(&mut self, addr: u16, value: u8) {
        self.bytes[addr as usize] = value;
    }
    fn input(&mut self, _port: u16) -> u8 {
        0
    }
    fn output(&mut self, port: u16, value: u8) {
        self.ports.push((port, value));
    }
}

/// Assemble `source`, run it until it halts, and hand back the final state.
fn run(source: &str) -> (Cpu, Memory) {
    let result = assemble("program.asm", source, &NoIncludes);
    assert!(
        result.succeeded(),
        "assembly failed: {}",
        result.errors.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")
    );

    let mut memory = Memory {
        bytes: Box::new([0; 0x10000]),
        ports: Vec::new(),
    };
    for (address, bytes) in result.segments() {
        memory.bytes[address as usize..address as usize + bytes.len()].copy_from_slice(&bytes);
    }

    let mut cpu = Cpu::new(false);
    cpu.regs.pc = result.origin;
    cpu.regs.sp = 0xFF00;

    // A generous ceiling: any of these programs finishes in far fewer steps.
    for _ in 0..200_000 {
        if cpu.step(&mut memory).halted {
            return (cpu, memory);
        }
    }
    panic!("program did not halt");
}

#[test]
fn sums_a_table_of_bytes() {
    let (cpu, _) = run(
        "        ORG  100h
COUNT   EQU  5
START   LD   HL,TABLE
        LD   B,COUNT
        XOR  A
LOOP    ADD  A,(HL)
        INC  HL
        DJNZ LOOP
        LD   (TOTAL),A
        HALT
TABLE   DB   1,2,3,4,5
TOTAL   DS   1
        END
",
    );
    assert_eq!(cpu.regs.a, 15);
}

#[test]
fn block_copy_and_string_data() {
    let (cpu, mut memory) = run(
        "        ORG  200h
        LD   HL,SOURCE
        LD   DE,DEST
        LD   BC,LENGTH
        LDIR
        HALT
SOURCE  DB   'Z80',0
LENGTH  EQU  4
DEST    DS   4
",
    );

    let dest = cpu
        .regs
        .de()
        .wrapping_sub(1);
    let copied: Vec<u8> = (0..3).map(|i| memory.read(dest - 3 + i)).collect();
    assert_eq!(copied, b"Z80");
}

#[test]
fn indexed_addressing_and_ports() {
    let (_, memory) = run(
        "        ORG  0
        LD   IX,BUFFER
        LD   (IX+0),41h
        LD   (IX+1),42h
        LD   A,(IX+1)
        OUT  (80h),A
        HALT
BUFFER  DS   2
",
    );
    assert_eq!(memory.ports, vec![(0x4280, 0x42)]);
}

#[test]
fn forward_references_resolve() {
    let (cpu, _) = run(
        "        ORG  300h
        JP   AHEAD
        LD   A,0FFh
AHEAD   LD   A,7
        HALT
",
    );
    assert_eq!(cpu.regs.a, 7);
}

#[test]
fn reports_errors_with_line_numbers() {
    let result = assemble(
        "bad.asm",
        "        ORG 100h\n        LD   A,MISSING\n        JR   9000h\n",
        &NoIncludes,
    );
    assert!(!result.succeeded());
    let messages: Vec<String> = result.errors.iter().map(ToString::to_string).collect();
    assert!(
        messages.iter().any(|m| m.contains("bad.asm(2)") && m.contains("MISSING")),
        "expected an undefined-symbol error on line 2, got {messages:?}"
    );
    assert!(
        messages.iter().any(|m| m.contains("bad.asm(3)") && m.contains("out of range")),
        "expected a relative-range error on line 3, got {messages:?}"
    );
}
