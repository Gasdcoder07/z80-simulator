//! Instruction encoding: mnemonic plus operands to machine code.

use crate::error::{to_byte, to_displacement, to_word, AsmError, AsmResult};
use crate::expr::EvalContext;
use crate::operand::{
    condition_code, index_half, index_prefix, is_index_register, reg16_code, reg8_code,
    stack_pair_code, MemRef, Operand,
};

/// Where an 8-bit operand lives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    /// One of B, C, D, E, H, L, A.
    Reg(u8),
    /// An undocumented half of IX or IY: prefix byte plus the H/L code it
    /// stands in for.
    Half(u8, u8),
    /// `(HL)`
    Memory,
    /// `(IX+d)` or `(IY+d)`.
    Indexed(u8, u8),
}

impl Target {
    fn code(self) -> u8 {
        match self {
            Target::Reg(code) | Target::Half(_, code) => code,
            Target::Memory | Target::Indexed(..) => 6,
        }
    }

    fn prefix(self) -> Option<u8> {
        match self {
            Target::Half(prefix, _) | Target::Indexed(prefix, _) => Some(prefix),
            _ => None,
        }
    }

    fn displacement(self) -> Option<u8> {
        match self {
            Target::Indexed(_, d) => Some(d),
            _ => None,
        }
    }

    fn reaches_memory(self) -> bool {
        matches!(self, Target::Memory | Target::Indexed(..))
    }
}

/// Bytes of one assembled instruction.
pub type Encoded = Vec<u8>;

/// Encode one instruction. `ctx.pc` must be the address the instruction is
/// being assembled at, since `JR` and `$` depend on it.
pub fn encode(
    mnemonic: &str,
    operands: &[Operand<'_>],
    ctx: &EvalContext<'_>,
) -> AsmResult<Encoded> {
    let bad = || AsmError::BadOperands { mnemonic: mnemonic.to_string() };

    // Operands, with a convenient empty stand-in for the missing ones.
    let empty = Operand::new(&[]);
    let first = operands.first().copied().unwrap_or(empty);
    let second = operands.get(1).copied().unwrap_or(empty);
    let count = operands.iter().filter(|op| !op.is_empty()).count();

    match mnemonic {
        // ---- no-operand instructions ---------------------------------
        "NOP" => simple(count, 0, &[0x00], bad),
        "HALT" => simple(count, 0, &[0x76], bad),
        "DI" => simple(count, 0, &[0xF3], bad),
        "EI" => simple(count, 0, &[0xFB], bad),
        "DAA" => simple(count, 0, &[0x27], bad),
        "CPL" => simple(count, 0, &[0x2F], bad),
        "CCF" => simple(count, 0, &[0x3F], bad),
        "SCF" => simple(count, 0, &[0x37], bad),
        "NEG" => simple(count, 0, &[0xED, 0x44], bad),
        "RLCA" => simple(count, 0, &[0x07], bad),
        "RRCA" => simple(count, 0, &[0x0F], bad),
        "RLA" => simple(count, 0, &[0x17], bad),
        "RRA" => simple(count, 0, &[0x1F], bad),
        "RLD" => simple(count, 0, &[0xED, 0x6F], bad),
        "RRD" => simple(count, 0, &[0xED, 0x67], bad),
        "EXX" => simple(count, 0, &[0xD9], bad),
        "RETI" => simple(count, 0, &[0xED, 0x4D], bad),
        "RETN" => simple(count, 0, &[0xED, 0x45], bad),
        "LDI" => simple(count, 0, &[0xED, 0xA0], bad),
        "LDIR" => simple(count, 0, &[0xED, 0xB0], bad),
        "LDD" => simple(count, 0, &[0xED, 0xA8], bad),
        "LDDR" => simple(count, 0, &[0xED, 0xB8], bad),
        "CPI" => simple(count, 0, &[0xED, 0xA1], bad),
        "CPIR" => simple(count, 0, &[0xED, 0xB1], bad),
        "CPD" => simple(count, 0, &[0xED, 0xA9], bad),
        "CPDR" => simple(count, 0, &[0xED, 0xB9], bad),
        "INI" => simple(count, 0, &[0xED, 0xA2], bad),
        "INIR" => simple(count, 0, &[0xED, 0xB2], bad),
        "IND" => simple(count, 0, &[0xED, 0xAA], bad),
        "INDR" => simple(count, 0, &[0xED, 0xBA], bad),
        "OUTI" => simple(count, 0, &[0xED, 0xA3], bad),
        "OTIR" => simple(count, 0, &[0xED, 0xB3], bad),
        "OUTD" => simple(count, 0, &[0xED, 0xAB], bad),
        "OTDR" => simple(count, 0, &[0xED, 0xBB], bad),

        "IM" => {
            let mode = first.value(ctx)?;
            let opcode = match mode {
                0 => 0x46,
                1 => 0x56,
                2 => 0x5E,
                _ => return Err(bad()),
            };
            Ok(vec![0xED, opcode])
        }

        "LD" => encode_ld(first, second, ctx).ok_or_else(bad)?,
        "EX" => encode_ex(first, second).ok_or_else(bad),
        "PUSH" | "POP" => encode_stack(mnemonic, first).ok_or_else(bad),

        "ADD" | "ADC" | "SBC" => encode_add_family(mnemonic, first, second, ctx, count),
        "SUB" | "AND" | "OR" | "XOR" | "CP" => {
            // These take one operand, but `SUB A,n` is also widely written.
            let operand = if count == 2 && first.name() == Some("A") { second } else { first };
            let op = alu_group(mnemonic).ok_or_else(bad)?;
            encode_alu(op, operand, ctx)
        }

        "INC" | "DEC" => encode_inc_dec(mnemonic, first, ctx),

        "RLC" | "RRC" | "RL" | "RR" | "SLA" | "SRA" | "SLL" | "SLI" | "SRL" => {
            let group = shift_group(mnemonic).ok_or_else(bad)?;
            encode_cb(group << 3, first, second, ctx, count)
        }
        "BIT" | "RES" | "SET" => {
            let base = match mnemonic {
                "BIT" => 0x40,
                "RES" => 0x80,
                _ => 0xC0,
            };
            let bit = first.value(ctx)?;
            if !(0..=7).contains(&bit) {
                return Err(AsmError::Range { value: bit, bits: 3 });
            }
            let target = operands.get(1).copied().unwrap_or(empty);
            let extra = operands.get(2).copied().unwrap_or(empty);
            encode_cb(base | ((bit as u8) << 3), target, extra, ctx, count - 1)
        }

        "JP" => encode_jp(first, second, ctx, count),
        "JR" => encode_jr(first, second, ctx, count),
        "DJNZ" => {
            let target = if count == 2 { second } else { first };
            Ok(vec![0x10, relative(target.value(ctx)?, ctx.pc, 2)?])
        }
        "CALL" => encode_call(first, second, ctx, count),
        "RET" => encode_ret(first, count),
        "RST" => {
            let target = first.value(ctx)?;
            if target & !0x38 != 0 {
                return Err(bad());
            }
            Ok(vec![0xC7 | target as u8])
        }

        "IN" => encode_in(first, second, ctx, count).ok_or_else(bad)?,
        "OUT" => encode_out(first, second, ctx, count).ok_or_else(bad)?,

        other => Err(AsmError::UnknownMnemonic(other.to_string())),
    }
}

fn simple(
    count: usize,
    expected: usize,
    bytes: &[u8],
    bad: impl Fn() -> AsmError,
) -> AsmResult<Encoded> {
    if count == expected {
        Ok(bytes.to_vec())
    } else {
        Err(bad())
    }
}

/// Evaluate a bracketed address expression as a 16-bit value.
fn word_operand(tokens: &[crate::lexer::Token], ctx: &EvalContext<'_>) -> AsmResult<u16> {
    to_word(crate::operand::evaluate(tokens, ctx)?)
}

/// Classify an operand as an 8-bit source or destination.
fn target_of(operand: Operand<'_>, ctx: &EvalContext<'_>) -> AsmResult<Option<Target>> {
    if let Some(name) = operand.name() {
        if let Some(code) = reg8_code(name) {
            return Ok(Some(Target::Reg(code)));
        }
        if let Some((prefix, code)) = index_half(name) {
            return Ok(Some(Target::Half(prefix, code)));
        }
        return Ok(None);
    }

    match operand.memory() {
        Some(MemRef::Register(name)) if name == "HL" => Ok(Some(Target::Memory)),
        Some(MemRef::Register(name)) if is_index_register(&name) => {
            // `(IX)` with no displacement is shorthand for `(IX+0)`.
            Ok(Some(Target::Indexed(index_prefix(&name).unwrap(), 0)))
        }
        Some(MemRef::Indexed(name, tokens)) => {
            let displacement = crate::operand::evaluate(tokens, ctx)?;
            Ok(Some(Target::Indexed(
                index_prefix(&name).unwrap(),
                to_displacement(displacement)?,
            )))
        }
        _ => Ok(None),
    }
}

/// Merge the index prefixes of two operands, rejecting `LD IXH,IYL` and
/// `LD (IX+0),IXH`, neither of which the hardware can encode.
fn merge(dst: Target, src: Target) -> Option<Option<u8>> {
    match (dst.prefix(), src.prefix()) {
        (None, None) => Some(None),
        (Some(p), None) | (None, Some(p)) => Some(Some(p)),
        (Some(a), Some(b)) if a == b && !dst.reaches_memory() && !src.reaches_memory() => {
            Some(Some(a))
        }
        _ => None,
    }
}

fn emit_indexed(bytes: &mut Encoded, prefix: Option<u8>, opcode: u8, displacement: Option<u8>) {
    if let Some(prefix) = prefix {
        bytes.push(prefix);
    }
    bytes.push(opcode);
    if let Some(displacement) = displacement {
        bytes.push(displacement);
    }
}

fn encode_ld(
    dst: Operand<'_>,
    src: Operand<'_>,
    ctx: &EvalContext<'_>,
) -> Option<AsmResult<Encoded>> {
    let dst_name = dst.name();
    let src_name = src.name();

    // ---- 16-bit forms, checked before the generic 8-bit path ----------
    if let (Some(d), Some(s)) = (dst_name, src_name) {
        if d == "SP" && (s == "HL" || is_index_register(s)) {
            let mut bytes = Vec::new();
            emit_indexed(&mut bytes, index_prefix(s), 0xF9, None);
            return Some(Ok(bytes));
        }
        match (d, s) {
            ("A", "I") => return Some(Ok(vec![0xED, 0x57])),
            ("A", "R") => return Some(Ok(vec![0xED, 0x5F])),
            ("I", "A") => return Some(Ok(vec![0xED, 0x47])),
            ("R", "A") => return Some(Ok(vec![0xED, 0x4F])),
            _ => {}
        }
    }

    // LD rr,(nn) and LD A,(BC)/(DE)
    if let (Some(d), Some(mem)) = (dst_name, src.memory()) {
        match (&mem, d) {
            (MemRef::Register(reg), "A") if reg == "BC" => return Some(Ok(vec![0x0A])),
            (MemRef::Register(reg), "A") if reg == "DE" => return Some(Ok(vec![0x1A])),
            (MemRef::Address(tokens), _) => {
                let address = match word_operand(tokens, ctx) {
                    Ok(address) => address,
                    Err(error) => return Some(Err(error)),
                };
                let [lo, hi] = address.to_le_bytes();
                if d == "A" {
                    return Some(Ok(vec![0x3A, lo, hi]));
                }
                if d == "HL" {
                    return Some(Ok(vec![0x2A, lo, hi]));
                }
                if is_index_register(d) {
                    return Some(Ok(vec![index_prefix(d).unwrap(), 0x2A, lo, hi]));
                }
                if let Some(code) = reg16_code(d) {
                    return Some(Ok(vec![0xED, 0x4B | (code << 4), lo, hi]));
                }
            }
            _ => {}
        }
    }

    // LD (nn),rr and LD (BC)/(DE),A
    if let (Some(mem), Some(s)) = (dst.memory(), src_name) {
        match (&mem, s) {
            (MemRef::Register(reg), "A") if reg == "BC" => return Some(Ok(vec![0x02])),
            (MemRef::Register(reg), "A") if reg == "DE" => return Some(Ok(vec![0x12])),
            (MemRef::Address(tokens), _) => {
                let address = match word_operand(tokens, ctx) {
                    Ok(address) => address,
                    Err(error) => return Some(Err(error)),
                };
                let [lo, hi] = address.to_le_bytes();
                if s == "A" {
                    return Some(Ok(vec![0x32, lo, hi]));
                }
                if s == "HL" {
                    return Some(Ok(vec![0x22, lo, hi]));
                }
                if is_index_register(s) {
                    return Some(Ok(vec![index_prefix(s).unwrap(), 0x22, lo, hi]));
                }
                if let Some(code) = reg16_code(s) {
                    return Some(Ok(vec![0xED, 0x43 | (code << 4), lo, hi]));
                }
            }
            _ => {}
        }
    }

    // LD rr,nn
    if let Some(d) = dst_name {
        if src.memory().is_none() && !src.is_empty() {
            let pair = if is_index_register(d) { Some(2) } else { reg16_code(d) };
            if let Some(code) = pair {
                if reg8_code(d).is_none() && index_half(d).is_none() {
                    let value = match src.value(ctx).map_err(AsmError::from).and_then(|v| to_word(v)) {
                        Ok(value) => value,
                        Err(error) => return Some(Err(error)),
                    };
                    let [lo, hi] = value.to_le_bytes();
                    let mut bytes = Vec::new();
                    emit_indexed(&mut bytes, index_prefix(d), 0x01 | (code << 4), None);
                    bytes.push(lo);
                    bytes.push(hi);
                    return Some(Ok(bytes));
                }
            }
        }
    }

    // ---- generic 8-bit loads ------------------------------------------
    let dst_target = match target_of(dst, ctx) {
        Ok(target) => target?,
        Err(error) => return Some(Err(error)),
    };

    let src_target = match target_of(src, ctx) {
        Ok(target) => target,
        Err(error) => return Some(Err(error)),
    };

    // LD r,n - anything that is not a register or a memory reference is an
    // immediate value, including a bare symbol name.
    let Some(src_target) = src_target else {
        if src.memory().is_some() || src.is_empty() {
            return None;
        }
        let value = match src.value(ctx).map_err(AsmError::from).and_then(to_byte) {
            Ok(value) => value,
            Err(error) => return Some(Err(error)),
        };
        let mut bytes = Vec::new();
        emit_indexed(
            &mut bytes,
            dst_target.prefix(),
            0x06 | (dst_target.code() << 3),
            dst_target.displacement(),
        );
        bytes.push(value);
        return Some(Ok(bytes));
    };

    // `LD (HL),(HL)` would encode as HALT, so it is not a load at all.
    if dst_target.reaches_memory() && src_target.reaches_memory() {
        return None;
    }

    let prefix = merge(dst_target, src_target)?;
    let mut bytes = Vec::new();
    emit_indexed(
        &mut bytes,
        prefix,
        0x40 | (dst_target.code() << 3) | src_target.code(),
        dst_target.displacement().or(src_target.displacement()),
    );
    Some(Ok(bytes))
}

fn encode_ex(first: Operand<'_>, second: Operand<'_>) -> Option<Encoded> {
    if first.name() == Some("DE") && second.name() == Some("HL") {
        return Some(vec![0xEB]);
    }
    if first.name() == Some("AF") && second.is_shadow_af() {
        return Some(vec![0x08]);
    }
    if let (Some(MemRef::Register(reg)), Some(name)) = (first.memory(), second.name()) {
        if reg == "SP" {
            let mut bytes = Vec::new();
            if name == "HL" {
                bytes.push(0xE3);
                return Some(bytes);
            }
            if is_index_register(name) {
                return Some(vec![index_prefix(name).unwrap(), 0xE3]);
            }
        }
    }
    None
}

fn encode_stack(mnemonic: &str, operand: Operand<'_>) -> Option<Encoded> {
    let name = operand.name()?;
    let base = if mnemonic == "PUSH" { 0xC5 } else { 0xC1 };
    if is_index_register(name) {
        // The index register takes HL's pair code, behind its prefix.
        return Some(vec![index_prefix(name).unwrap(), base | (2 << 4)]);
    }
    let code = stack_pair_code(name)?;
    Some(vec![base | (code << 4)])
}

fn alu_group(mnemonic: &str) -> Option<u8> {
    Some(match mnemonic {
        "ADD" => 0,
        "ADC" => 1,
        "SUB" => 2,
        "SBC" => 3,
        "AND" => 4,
        "XOR" => 5,
        "OR" => 6,
        "CP" => 7,
        _ => return None,
    })
}

fn shift_group(mnemonic: &str) -> Option<u8> {
    Some(match mnemonic {
        "RLC" => 0,
        "RRC" => 1,
        "RL" => 2,
        "RR" => 3,
        "SLA" => 4,
        "SRA" => 5,
        "SLL" | "SLI" => 6,
        "SRL" => 7,
        _ => return None,
    })
}

/// `ADD`, `ADC` and `SBC` are the only mnemonics with both 8- and 16-bit
/// forms, so they need the destination to disambiguate.
fn encode_add_family(
    mnemonic: &str,
    first: Operand<'_>,
    second: Operand<'_>,
    ctx: &EvalContext<'_>,
    count: usize,
) -> AsmResult<Encoded> {
    let bad = || AsmError::BadOperands { mnemonic: mnemonic.to_string() };
    let group = alu_group(mnemonic).ok_or_else(bad)?;

    if count == 2 {
        if let (Some(dst), Some(src)) = (first.name(), second.name()) {
            // 16-bit: ADD HL,rr / ADD IX,pp / ADC HL,rr / SBC HL,rr
            if dst == "HL" || is_index_register(dst) {
                // Under a prefix the pair code for HL means the index
                // register itself, so `ADD IX,IX` is legal but `ADD IX,HL`
                // is not.
                let code = if src == dst {
                    2
                } else if is_index_register(src) || src == "HL" {
                    return Err(bad());
                } else {
                    reg16_code(src).ok_or_else(bad)?
                };

                return match (mnemonic, index_prefix(dst)) {
                    ("ADD", None) => Ok(vec![0x09 | (code << 4)]),
                    ("ADD", Some(prefix)) => Ok(vec![prefix, 0x09 | (code << 4)]),
                    ("ADC", None) => Ok(vec![0xED, 0x4A | (code << 4)]),
                    ("SBC", None) => Ok(vec![0xED, 0x42 | (code << 4)]),
                    _ => Err(bad()),
                };
            }
        }
    }

    // 8-bit: the accumulator is implied when it is written out.
    let operand = if count == 2 && first.name() == Some("A") { second } else { first };
    if count == 2 && first.name() != Some("A") {
        return Err(bad());
    }
    encode_alu(group, operand, ctx)
}

fn encode_alu(group: u8, operand: Operand<'_>, ctx: &EvalContext<'_>) -> AsmResult<Encoded> {
    if let Some(target) = target_of(operand, ctx)? {
        let mut bytes = Vec::new();
        emit_indexed(
            &mut bytes,
            target.prefix(),
            0x80 | (group << 3) | target.code(),
            target.displacement(),
        );
        return Ok(bytes);
    }

    // Immediate form.
    let value = to_byte(operand.value(ctx)?)?;
    Ok(vec![0xC6 | (group << 3), value])
}

fn encode_inc_dec(
    mnemonic: &str,
    operand: Operand<'_>,
    ctx: &EvalContext<'_>,
) -> AsmResult<Encoded> {
    let bad = || AsmError::BadOperands { mnemonic: mnemonic.to_string() };
    let low_bit = if mnemonic == "INC" { 0x04 } else { 0x05 };

    // 16-bit registers first: `INC HL` is not `INC H`.
    if let Some(name) = operand.name() {
        if reg8_code(name).is_none() && index_half(name).is_none() {
            let opcode = if mnemonic == "INC" { 0x03 } else { 0x0B };
            if is_index_register(name) {
                return Ok(vec![index_prefix(name).unwrap(), opcode | (2 << 4)]);
            }
            let code = reg16_code(name).ok_or_else(bad)?;
            return Ok(vec![opcode | (code << 4)]);
        }
    }

    let target = target_of(operand, ctx)?.ok_or_else(bad)?;
    let mut bytes = Vec::new();
    emit_indexed(
        &mut bytes,
        target.prefix(),
        low_bit | (target.code() << 3),
        target.displacement(),
    );
    Ok(bytes)
}

/// Shifts, rotates and bit operations, including the undocumented
/// `RLC (IX+d),B` form that also copies the result into a register.
fn encode_cb(
    opcode_base: u8,
    target_operand: Operand<'_>,
    copy_operand: Operand<'_>,
    ctx: &EvalContext<'_>,
    count: usize,
) -> AsmResult<Encoded> {
    let bad = || AsmError::BadOperands { mnemonic: "CB-prefixed instruction".to_string() };
    let target = target_of(target_operand, ctx)?.ok_or_else(bad)?;

    let register_code = if count == 2 {
        // `SET 0,(IX+d),B` - the result also lands in B.
        let copy = target_of(copy_operand, ctx)?.ok_or_else(bad)?;
        match (target, copy) {
            (Target::Indexed(..), Target::Reg(code)) => code,
            _ => return Err(bad()),
        }
    } else {
        target.code()
    };

    match target {
        Target::Indexed(prefix, displacement) => {
            Ok(vec![prefix, 0xCB, displacement, opcode_base | register_code])
        }
        Target::Half(..) => Err(bad()),
        _ => Ok(vec![0xCB, opcode_base | target.code()]),
    }
}

fn encode_jp(
    first: Operand<'_>,
    second: Operand<'_>,
    ctx: &EvalContext<'_>,
    count: usize,
) -> AsmResult<Encoded> {
    // JP (HL) / JP (IX) / JP (IY). Some sources write these without the
    // parentheses, which is accepted here too.
    let indirect = first
        .memory()
        .and_then(|mem| match mem {
            MemRef::Register(name) => Some(name),
            _ => None,
        })
        .or_else(|| first.name().map(str::to_string))
        .filter(|name| name == "HL" || is_index_register(name));

    if count == 1 {
        if let Some(name) = indirect {
            return Ok(match index_prefix(&name) {
                Some(prefix) => vec![prefix, 0xE9],
                None => vec![0xE9],
            });
        }
        let [lo, hi] = to_word(first.value(ctx)?)?.to_le_bytes();
        return Ok(vec![0xC3, lo, hi]);
    }

    let condition = first
        .name()
        .and_then(condition_code)
        .ok_or(AsmError::BadOperands { mnemonic: "JP".into() })?;
    let [lo, hi] = to_word(second.value(ctx)?)?.to_le_bytes();
    Ok(vec![0xC2 | (condition << 3), lo, hi])
}

fn encode_jr(
    first: Operand<'_>,
    second: Operand<'_>,
    ctx: &EvalContext<'_>,
    count: usize,
) -> AsmResult<Encoded> {
    if count == 1 {
        return Ok(vec![0x18, relative(first.value(ctx)?, ctx.pc, 2)?]);
    }

    // Only the four flag conditions reachable by a relative jump.
    let condition = first
        .name()
        .and_then(condition_code)
        .filter(|code| *code < 4)
        .ok_or(AsmError::BadOperands { mnemonic: "JR".into() })?;
    Ok(vec![0x20 | (condition << 3), relative(second.value(ctx)?, ctx.pc, 2)?])
}

fn encode_call(
    first: Operand<'_>,
    second: Operand<'_>,
    ctx: &EvalContext<'_>,
    count: usize,
) -> AsmResult<Encoded> {
    if count == 1 {
        let [lo, hi] = to_word(first.value(ctx)?)?.to_le_bytes();
        return Ok(vec![0xCD, lo, hi]);
    }
    let condition = first
        .name()
        .and_then(condition_code)
        .ok_or(AsmError::BadOperands { mnemonic: "CALL".into() })?;
    let [lo, hi] = to_word(second.value(ctx)?)?.to_le_bytes();
    Ok(vec![0xC4 | (condition << 3), lo, hi])
}

fn encode_ret(first: Operand<'_>, count: usize) -> AsmResult<Encoded> {
    if count == 0 {
        return Ok(vec![0xC9]);
    }
    let condition = first
        .name()
        .and_then(condition_code)
        .ok_or(AsmError::BadOperands { mnemonic: "RET".into() })?;
    Ok(vec![0xC0 | (condition << 3)])
}

fn encode_in(
    first: Operand<'_>,
    second: Operand<'_>,
    ctx: &EvalContext<'_>,
    count: usize,
) -> Option<AsmResult<Encoded>> {
    if count != 2 {
        return None;
    }
    let port = second.memory()?;
    let destination = first.name()?;

    match port {
        // IN r,(C) - and the undocumented `IN F,(C)`, which only sets flags.
        MemRef::Register(name) if name == "C" => {
            let code = match destination {
                "F" => 6,
                name => reg8_code(name)?,
            };
            Some(Ok(vec![0xED, 0x40 | (code << 3)]))
        }
        // IN A,(n)
        MemRef::Address(tokens) if destination == "A" => {
            let value = crate::operand::evaluate(tokens, ctx)
                .map_err(AsmError::from)
                .and_then(to_byte);
            Some(value.map(|value| vec![0xDB, value]))
        }
        _ => None,
    }
}

fn encode_out(
    first: Operand<'_>,
    second: Operand<'_>,
    ctx: &EvalContext<'_>,
    count: usize,
) -> Option<AsmResult<Encoded>> {
    if count != 2 {
        return None;
    }
    let port = first.memory()?;

    match port {
        MemRef::Register(name) if name == "C" => {
            // `OUT (C),0` is the undocumented zero-output form.
            let code = match second.name() {
                Some(name) => reg8_code(name)?,
                None => match second.value(ctx) {
                    Ok(0) => 6,
                    _ => return None,
                },
            };
            Some(Ok(vec![0xED, 0x41 | (code << 3)]))
        }
        MemRef::Address(tokens) if second.name() == Some("A") => {
            let value = crate::operand::evaluate(tokens, ctx)
                .map_err(AsmError::from)
                .and_then(to_byte);
            Some(value.map(|value| vec![0xD3, value]))
        }
        _ => None,
    }
}

/// Displacement for a relative jump, measured from the address after the
/// two-byte instruction.
fn relative(target: i64, pc: u16, instruction_length: i64) -> AsmResult<u8> {
    let distance = target - (pc as i64 + instruction_length);
    if (-128..=127).contains(&distance) {
        Ok(distance as u8)
    } else {
        Err(AsmError::RelativeRange { distance })
    }
}

/// Every mnemonic the encoder understands. Used to tell a label at the left
/// margin from an unindented instruction.
pub const MNEMONICS: &[&str] = &[
    "ADC", "ADD", "AND", "BIT", "CALL", "CCF", "CP", "CPD", "CPDR", "CPI", "CPIR", "CPL",
    "DAA", "DEC", "DI", "DJNZ", "EI", "EX", "EXX", "HALT", "IM", "IN", "INC", "IND",
    "INDR", "INI", "INIR", "JP", "JR", "LD", "LDD", "LDDR", "LDI", "LDIR", "NEG", "NOP",
    "OR", "OTDR", "OTIR", "OUT", "OUTD", "OUTI", "POP", "PUSH", "RES", "RET", "RETI",
    "RETN", "RL", "RLA", "RLC", "RLCA", "RLD", "RR", "RRA", "RRC", "RRCA", "RRD", "RST",
    "SBC", "SCF", "SET", "SLA", "SLI", "SLL", "SRA", "SRL", "SUB", "XOR",
];

pub fn is_mnemonic(word: &str) -> bool {
    MNEMONICS.contains(&word)
}
