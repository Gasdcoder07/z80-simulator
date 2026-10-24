//! Flag bit definitions and the ALU helpers that compute them.
//!
//! Flags 3 and 5 have no documented meaning but are observable, and the
//! conformance suites (z80flags, ZEXALL) check them, so every operation here
//! sets them exactly as the hardware does.

/// Sign: bit 7 of the result.
pub const S: u8 = 0b1000_0000;
/// Zero.
pub const Z: u8 = 0b0100_0000;
/// Undocumented flag 5: copy of bit 5 of the result.
pub const F5: u8 = 0b0010_0000;
/// Half carry (carry out of bit 3).
pub const H: u8 = 0b0001_0000;
/// Undocumented flag 3: copy of bit 3 of the result.
pub const F3: u8 = 0b0000_1000;
/// Parity / overflow, depending on the instruction.
pub const PV: u8 = 0b0000_0100;
/// Add/Subtract: set by subtractions, used by `DAA`.
pub const N: u8 = 0b0000_0010;
/// Carry.
pub const C: u8 = 0b0000_0001;

/// Flags 3 and 5 together, which many operations copy straight from a result.
pub const F3F5: u8 = F3 | F5;

const fn build_parity() -> [u8; 256] {
    let mut table = [0u8; 256];
    let mut i = 0;
    while i < 256 {
        table[i] = if (i as u8).count_ones() % 2 == 0 { PV } else { 0 };
        i += 1;
    }
    table
}

/// `PARITY[v]` is `PV` when `v` has an even number of set bits, else 0.
pub static PARITY: [u8; 256] = build_parity();

/// S, Z, F3 and F5 as implied by a byte value.
#[inline]
pub fn sz53(value: u8) -> u8 {
    (value & (S | F3F5)) | if value == 0 { Z } else { 0 }
}

/// S, Z, F3, F5 and parity as implied by a byte value.
#[inline]
pub fn sz53p(value: u8) -> u8 {
    sz53(value) | PARITY[value as usize]
}

/// `ADD A,n` / `ADC A,n`. `carry_in` is 0 or 1.
#[inline]
pub fn add8(a: u8, operand: u8, carry_in: u8) -> (u8, u8) {
    let wide = a as u16 + operand as u16 + carry_in as u16;
    let result = wide as u8;
    let half = (a & 0x0F) + (operand & 0x0F) + carry_in;
    let overflow = (a ^ result) & (operand ^ result) & 0x80;
    let flags = sz53(result)
        | if half > 0x0F { H } else { 0 }
        | if overflow != 0 { PV } else { 0 }
        | if wide > 0xFF { C } else { 0 };
    (result, flags)
}

/// `SUB n` / `SBC A,n` / the arithmetic behind `CP n`.
#[inline]
pub fn sub8(a: u8, operand: u8, carry_in: u8) -> (u8, u8) {
    let wide = (a as i16) - (operand as i16) - (carry_in as i16);
    let result = wide as u8;
    let half = (a as i16 & 0x0F) - (operand as i16 & 0x0F) - (carry_in as i16);
    let overflow = (a ^ operand) & (a ^ result) & 0x80;
    let flags = sz53(result)
        | N
        | if half < 0 { H } else { 0 }
        | if overflow != 0 { PV } else { 0 }
        | if wide < 0 { C } else { 0 };
    (result, flags)
}

/// `CP n`: like `SUB` but flags 3 and 5 come from the operand, not the result,
/// because the result is discarded and never reaches the flag latch.
#[inline]
pub fn cp8(a: u8, operand: u8) -> u8 {
    let (_, flags) = sub8(a, operand, 0);
    (flags & !F3F5) | (operand & F3F5)
}

/// `INC r`: carry is preserved.
#[inline]
pub fn inc8(value: u8, flags: u8) -> (u8, u8) {
    let result = value.wrapping_add(1);
    let new = (flags & C)
        | sz53(result)
        | if result & 0x0F == 0 { H } else { 0 }
        | if result == 0x80 { PV } else { 0 };
    (result, new)
}

/// `DEC r`: carry is preserved.
#[inline]
pub fn dec8(value: u8, flags: u8) -> (u8, u8) {
    let result = value.wrapping_sub(1);
    let new = (flags & C)
        | N
        | sz53(result)
        | if result & 0x0F == 0x0F { H } else { 0 }
        | if result == 0x7F { PV } else { 0 };
    (result, new)
}

/// `AND n`: the only logic op that sets the half-carry.
#[inline]
pub fn and8(a: u8, operand: u8) -> (u8, u8) {
    let result = a & operand;
    (result, sz53p(result) | H)
}

/// `OR n`
#[inline]
pub fn or8(a: u8, operand: u8) -> (u8, u8) {
    let result = a | operand;
    (result, sz53p(result))
}

/// `XOR n`
#[inline]
pub fn xor8(a: u8, operand: u8) -> (u8, u8) {
    let result = a ^ operand;
    (result, sz53p(result))
}

/// `ADD HL,rr`: S, Z and P/V are left alone; flags 3 and 5 come from the
/// high byte of the result.
#[inline]
pub fn add16(a: u16, operand: u16, flags: u8) -> (u16, u8) {
    let wide = a as u32 + operand as u32;
    let result = wide as u16;
    let half = (a & 0x0FFF) + (operand & 0x0FFF);
    let new = (flags & (S | Z | PV))
        | ((result >> 8) as u8 & F3F5)
        | if half > 0x0FFF { H } else { 0 }
        | if wide > 0xFFFF { C } else { 0 };
    (result, new)
}

/// `ADC HL,rr`: unlike `ADD HL,rr` this one sets every flag.
#[inline]
pub fn adc16(a: u16, operand: u16, carry_in: u8) -> (u16, u8) {
    let wide = a as u32 + operand as u32 + carry_in as u32;
    let result = wide as u16;
    let half = (a & 0x0FFF) + (operand & 0x0FFF) + carry_in as u16;
    let overflow = (a ^ result) & (operand ^ result) & 0x8000;
    let new = ((result >> 8) as u8 & (S | F3F5))
        | if result == 0 { Z } else { 0 }
        | if half > 0x0FFF { H } else { 0 }
        | if overflow != 0 { PV } else { 0 }
        | if wide > 0xFFFF { C } else { 0 };
    (result, new)
}

/// `SBC HL,rr`
#[inline]
pub fn sbc16(a: u16, operand: u16, carry_in: u8) -> (u16, u8) {
    let wide = (a as i32) - (operand as i32) - (carry_in as i32);
    let result = wide as u16;
    let half = (a as i32 & 0x0FFF) - (operand as i32 & 0x0FFF) - (carry_in as i32);
    let overflow = (a ^ operand) & (a ^ result) & 0x8000;
    let new = ((result >> 8) as u8 & (S | F3F5))
        | N
        | if result == 0 { Z } else { 0 }
        | if half < 0 { H } else { 0 }
        | if overflow != 0 { PV } else { 0 }
        | if wide < 0 { C } else { 0 };
    (result, new)
}

/// Shared tail for the `CB`-prefixed rotates and shifts: they all set
/// S, Z, parity, clear H and N, and take carry from the bit shifted out.
#[inline]
fn shift_flags(result: u8, carry_out: bool) -> u8 {
    sz53p(result) | if carry_out { C } else { 0 }
}

#[inline]
pub fn rlc(value: u8) -> (u8, u8) {
    let result = value.rotate_left(1);
    (result, shift_flags(result, value & 0x80 != 0))
}

#[inline]
pub fn rrc(value: u8) -> (u8, u8) {
    let result = value.rotate_right(1);
    (result, shift_flags(result, value & 0x01 != 0))
}

#[inline]
pub fn rl(value: u8, carry_in: u8) -> (u8, u8) {
    let result = (value << 1) | carry_in;
    (result, shift_flags(result, value & 0x80 != 0))
}

#[inline]
pub fn rr(value: u8, carry_in: u8) -> (u8, u8) {
    let result = (value >> 1) | (carry_in << 7);
    (result, shift_flags(result, value & 0x01 != 0))
}

#[inline]
pub fn sla(value: u8) -> (u8, u8) {
    let result = value << 1;
    (result, shift_flags(result, value & 0x80 != 0))
}

#[inline]
pub fn sra(value: u8) -> (u8, u8) {
    let result = (value >> 1) | (value & 0x80);
    (result, shift_flags(result, value & 0x01 != 0))
}

/// `SLL`: undocumented, shifts left and feeds in a 1.
#[inline]
pub fn sll(value: u8) -> (u8, u8) {
    let result = (value << 1) | 1;
    (result, shift_flags(result, value & 0x80 != 0))
}

#[inline]
pub fn srl(value: u8) -> (u8, u8) {
    let result = value >> 1;
    (result, shift_flags(result, value & 0x01 != 0))
}

/// `DAA`: adjusts A after a BCD add or subtract. `N` selects which.
#[inline]
pub fn daa(a: u8, flags: u8) -> (u8, u8) {
    let mut correction = 0u8;
    let mut carry = flags & C != 0;

    if flags & H != 0 || a & 0x0F > 9 {
        correction |= 0x06;
    }
    if carry || a > 0x99 {
        correction |= 0x60;
        carry = true;
    }

    let (result, half) = if flags & N != 0 {
        (
            a.wrapping_sub(correction),
            flags & H != 0 && a & 0x0F < 6,
        )
    } else {
        (a.wrapping_add(correction), a & 0x0F > 9)
    };

    let new = sz53p(result)
        | (flags & N)
        | if half { H } else { 0 }
        | if carry { C } else { 0 };
    (result, new)
}

/// `CPL`
#[inline]
pub fn cpl(a: u8, flags: u8) -> (u8, u8) {
    let result = !a;
    ((result), (flags & (S | Z | PV | C)) | H | N | (result & F3F5))
}

/// `NEG`: `SUB A` with the accumulator as the operand.
#[inline]
pub fn neg(a: u8) -> (u8, u8) {
    sub8(0, a, 0)
}

/// `SCF`. On a real Z80 flags 3 and 5 are OR-ed in from A, which is the
/// behaviour the Zilog NMOS parts show and what z80flags expects.
#[inline]
pub fn scf(a: u8, flags: u8) -> u8 {
    (flags & (S | Z | PV)) | C | ((a | flags) & F3F5)
}

/// `CCF`
#[inline]
pub fn ccf(a: u8, flags: u8) -> u8 {
    let carry = flags & C;
    (flags & (S | Z | PV))
        | if carry != 0 { H } else { 0 }
        | (carry ^ C)
        | ((a | flags) & F3F5)
}
