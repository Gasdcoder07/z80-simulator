//! Clock-cycle counts per opcode.
//!
//! The tables hold the cost of the *untaken* path for conditional
//! instructions; the execute functions add the extra cycles when a branch is
//! taken or a block instruction repeats. A `DD`/`FD` prefix adds 4 cycles per
//! prefix byte, and an indexed memory operand adds a further 8 for fetching
//! and applying the displacement.

/// Extra cycles for resolving `(IX+d)` / `(IY+d)` instead of `(HL)`.
pub(crate) const INDEX_DISPLACEMENT_CYCLES: u32 = 8;
/// Cycles charged for each `DD`, `FD`, `CB` or `ED` prefix byte fetched.
pub(crate) const PREFIX_CYCLES: u32 = 4;

const fn build_base() -> [u8; 256] {
    // Lay the table out one 16-opcode row at a time so it can be checked
    // against the Zilog manual by eye.
    let mut t: [u8; 256] = [4; 256];

    // 0x00-0x3F: the mixed block of loads, 16-bit arithmetic and jumps.
    let low: [u8; 64] = [
        4, 10, 7, 6, 4, 4, 7, 4, 4, 11, 7, 6, 4, 4, 7, 4, // 0x00
        8, 10, 7, 6, 4, 4, 7, 4, 12, 11, 7, 6, 4, 4, 7, 4, // 0x10  DJNZ=8(+5)
        7, 10, 16, 6, 4, 4, 7, 4, 7, 11, 16, 6, 4, 4, 7, 4, // 0x20  JR cc=7(+5)
        7, 10, 13, 6, 11, 11, 10, 4, 7, 11, 13, 6, 4, 4, 7, 4, // 0x30
    ];
    let mut i = 0;
    while i < 64 {
        t[i] = low[i];
        i += 1;
    }

    // 0x40-0x7F: LD r,r'. Any operand of 6 means (HL) and costs 7.
    i = 0x40;
    while i < 0x80 {
        let dst = (i >> 3) & 7;
        let src = i & 7;
        t[i] = if dst == 6 || src == 6 { 7 } else { 4 };
        i += 1;
    }
    t[0x76] = 4; // HALT, not LD (HL),(HL)

    // 0x80-0xBF: ALU A,r. Same rule for the (HL) operand.
    i = 0x80;
    while i < 0xC0 {
        t[i] = if i & 7 == 6 { 7 } else { 4 };
        i += 1;
    }

    // 0xC0-0xFF: four near-identical rows of stack, jump and call ops.
    let high: [u8; 64] = [
        5, 10, 10, 10, 10, 11, 7, 11, 5, 10, 10, 4, 10, 17, 7, 11, // 0xC0
        5, 10, 10, 11, 10, 11, 7, 11, 5, 4, 10, 11, 10, 4, 7, 11, // 0xD0
        5, 10, 10, 19, 10, 11, 7, 11, 5, 4, 10, 4, 10, 4, 7, 11, // 0xE0
        5, 10, 10, 4, 10, 11, 7, 11, 5, 6, 10, 4, 10, 4, 7, 11, // 0xF0
    ];
    i = 0;
    while i < 64 {
        t[0xC0 + i] = high[i];
        i += 1;
    }

    t
}

const fn build_ed() -> [u8; 256] {
    // Everything outside the implemented ranges behaves as two NOPs.
    let mut t: [u8; 256] = [8; 256];

    let mut i = 0x40;
    while i < 0x80 {
        t[i] = match i & 7 {
            0 | 1 => 12,     // IN r,(C) / OUT (C),r
            2 => 15,         // SBC/ADC HL,rr
            3 => 20,         // LD (nn),rr / LD rr,(nn)
            4 => 8,          // NEG
            5 => 14,         // RETN / RETI
            6 => 8,          // IM n
            _ => 9,          // LD I,A / LD R,A / LD A,I / LD A,R
        };
        i += 1;
    }
    t[0x67] = 18; // RRD
    t[0x6F] = 18; // RLD
    t[0x77] = 8;  // undefined, acts as NOP
    t[0x7F] = 8;

    // Block transfer, search and I/O. Columns 0-3 of rows A0 and B0 are
    // the real instructions; columns 4-7 are undefined and act as NOPs.
    // The repeating forms cost 5 more cycles on every iteration that loops.
    i = 0xA0;
    while i < 0xC0 {
        t[i] = if i & 7 < 4 { 16 } else { 8 };
        i += 1;
    }

    t
}

pub(crate) static BASE_CYCLES: [u8; 256] = build_base();
pub(crate) static ED_CYCLES: [u8; 256] = build_ed();

/// Cycles for a `CB`-prefixed opcode, before prefix and displacement costs.
#[inline]
pub(crate) const fn cb_cycles(opcode: u8) -> u32 {
    if opcode & 7 == 6 {
        // Operates on (HL): BIT only reads, the rest read and write back.
        if opcode & 0xC0 == 0x40 {
            12
        } else {
            15
        }
    } else {
        8
    }
}
