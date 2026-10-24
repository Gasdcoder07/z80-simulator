//! Disassembler.
//!
//! The text it produces is deliberately re-assemblable by this crate's own
//! assembler, which makes a round-trip test possible: assemble a program,
//! disassemble the bytes, assemble the result and compare.

/// One decoded instruction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Instruction {
    pub address: u16,
    pub bytes: Vec<u8>,
    pub text: String,
}

impl Instruction {
    pub fn len(&self) -> u16 {
        self.bytes.len() as u16
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// The address of the instruction that follows this one.
    pub fn next(&self) -> u16 {
        self.address.wrapping_add(self.len())
    }
}

const REG8: [&str; 8] = ["B", "C", "D", "E", "H", "L", "(HL)", "A"];
const PAIRS: [&str; 4] = ["BC", "DE", "HL", "SP"];
const STACK_PAIRS: [&str; 4] = ["BC", "DE", "HL", "AF"];
const CONDITIONS: [&str; 8] = ["NZ", "Z", "NC", "C", "PO", "PE", "P", "M"];
const ALU: [&str; 8] = [
    "ADD A,", "ADC A,", "SUB ", "SBC A,", "AND ", "XOR ", "OR ", "CP ",
];
const ROTATES: [&str; 8] = ["RLC", "RRC", "RL", "RR", "SLA", "SRA", "SLL", "SRL"];
const BLOCK: [[&str; 4]; 4] = [
    ["LDI", "CPI", "INI", "OUTI"],
    ["LDD", "CPD", "IND", "OUTD"],
    ["LDIR", "CPIR", "INIR", "OTIR"],
    ["LDDR", "CPDR", "INDR", "OTDR"],
];

/// Decode the instruction at `address`, reading bytes through `read`.
pub fn disassemble_one(read: &mut dyn FnMut(u16) -> u8, address: u16) -> Instruction {
    let mut decoder = Decoder { read, address, bytes: Vec::new() };
    let text = decoder.decode();
    Instruction { address, bytes: decoder.bytes, text }
}

/// Decode a run of instructions covering `[start, end)`.
pub fn disassemble_range(
    read: &mut dyn FnMut(u16) -> u8,
    start: u16,
    end: u32,
) -> Vec<Instruction> {
    let mut out = Vec::new();
    let mut address = start as u32;

    while address < end {
        let instruction = disassemble_one(read, address as u16);
        address += instruction.len() as u32;
        out.push(instruction);
    }

    out
}

struct Decoder<'a> {
    read: &'a mut dyn FnMut(u16) -> u8,
    address: u16,
    bytes: Vec<u8>,
}

impl Decoder<'_> {
    fn next_byte(&mut self) -> u8 {
        let addr = self.address.wrapping_add(self.bytes.len() as u16);
        let byte = (self.read)(addr);
        self.bytes.push(byte);
        byte
    }

    fn next_word(&mut self) -> u16 {
        let lo = self.next_byte();
        let hi = self.next_byte();
        u16::from_be_bytes([hi, lo])
    }

    fn decode(&mut self) -> String {
        let mut index: Option<&'static str> = None;

        loop {
            let opcode = self.next_byte();
            match opcode {
                0xDD => index = Some("IX"),
                0xFD => index = Some("IY"),
                0xCB => return self.decode_cb(index),
                0xED => return self.decode_ed(),
                _ => return self.decode_base(opcode, index),
            }
        }
    }

    /// Render an 8-bit operand slot, substituting the index register where
    /// a prefix applies. `memory_elsewhere` is set when the other operand of
    /// the instruction is already `(IX+d)`, in which case H and L stay put.
    fn operand(&mut self, code: u8, index: Option<&str>, memory_elsewhere: bool) -> String {
        match (code, index) {
            (6, Some(name)) => {
                let displacement = self.next_byte() as i8;
                format!("({name}{displacement:+})")
            }
            (4, Some(name)) if !memory_elsewhere => format!("{name}H"),
            (5, Some(name)) if !memory_elsewhere => format!("{name}L"),
            _ => REG8[code as usize].to_string(),
        }
    }

    fn pair(&self, code: u8, index: Option<&str>) -> String {
        match (code, index) {
            (2, Some(name)) => name.to_string(),
            _ => PAIRS[code as usize].to_string(),
        }
    }

    fn decode_base(&mut self, opcode: u8, index: Option<&str>) -> String {
        let hl = index.unwrap_or("HL");
        let x = opcode >> 6;
        let y = (opcode >> 3) & 7;
        let z = opcode & 7;

        match (x, z) {
            // ---- 0x00-0x3F -------------------------------------------
            (0, 0) => match y {
                0 => "NOP".into(),
                1 => "EX AF,AF'".into(),
                2 => format!("DJNZ {}", self.relative_target()),
                3 => format!("JR {}", self.relative_target()),
                _ => format!("JR {},{}", CONDITIONS[(y - 4) as usize], self.relative_target()),
            },
            (0, 1) if y & 1 == 0 => {
                let pair = self.pair(y >> 1, index);
                format!("LD {pair},{}", hex16(self.next_word()))
            }
            (0, 1) => format!("ADD {hl},{}", self.pair(y >> 1, index)),
            (0, 2) => match y {
                0 => "LD (BC),A".into(),
                1 => "LD A,(BC)".into(),
                2 => "LD (DE),A".into(),
                3 => "LD A,(DE)".into(),
                4 => format!("LD ({}),{hl}", hex16(self.next_word())),
                5 => format!("LD {hl},({})", hex16(self.next_word())),
                6 => format!("LD ({}),A", hex16(self.next_word())),
                _ => format!("LD A,({})", hex16(self.next_word())),
            },
            (0, 3) if y & 1 == 0 => format!("INC {}", self.pair(y >> 1, index)),
            (0, 3) => format!("DEC {}", self.pair(y >> 1, index)),
            (0, 4) => format!("INC {}", self.operand(y, index, false)),
            (0, 5) => format!("DEC {}", self.operand(y, index, false)),
            (0, 6) => {
                // The displacement comes before the immediate byte.
                let target = self.operand(y, index, false);
                format!("LD {target},{}", hex8(self.next_byte()))
            }
            (0, _) => ["RLCA", "RRCA", "RLA", "RRA", "DAA", "CPL", "SCF", "CCF"][y as usize]
                .to_string(),

            // ---- 0x40-0x7F: LD r,r' and HALT --------------------------
            (1, _) if y == 6 && z == 6 => "HALT".into(),
            (1, _) => {
                let memory = y == 6 || z == 6;
                // Whichever operand is `(HL)` must consume the displacement
                // first, so decode the destination before the source only
                // when the destination is the memory one.
                if y == 6 {
                    let dst = self.operand(y, index, memory);
                    let src = self.operand(z, index, memory);
                    format!("LD {dst},{src}")
                } else {
                    let src_is_memory = z == 6;
                    let (dst, src) = if src_is_memory {
                        let src = self.operand(z, index, memory);
                        (self.operand(y, index, memory), src)
                    } else {
                        (self.operand(y, index, memory), self.operand(z, index, memory))
                    };
                    format!("LD {dst},{src}")
                }
            }

            // ---- 0x80-0xBF: ALU ---------------------------------------
            (2, _) => format!("{}{}", ALU[y as usize], self.operand(z, index, false)),

            // ---- 0xC0-0xFF --------------------------------------------
            (_, 0) => format!("RET {}", CONDITIONS[y as usize]),
            (_, 1) if y & 1 == 0 => {
                let pair = match (y >> 1, index) {
                    (2, Some(name)) => name.to_string(),
                    (code, _) => STACK_PAIRS[code as usize].to_string(),
                };
                format!("POP {pair}")
            }
            (_, 1) => match y >> 1 {
                0 => "RET".into(),
                1 => "EXX".into(),
                2 => format!("JP ({hl})"),
                _ => format!("LD SP,{hl}"),
            },
            (_, 2) => format!("JP {},{}", CONDITIONS[y as usize], hex16(self.next_word())),
            (_, 3) => match y {
                0 => format!("JP {}", hex16(self.next_word())),
                2 => format!("OUT ({}),A", hex8(self.next_byte())),
                3 => format!("IN A,({})", hex8(self.next_byte())),
                4 => format!("EX (SP),{hl}"),
                5 => "EX DE,HL".into(),
                6 => "DI".into(),
                7 => "EI".into(),
                // y == 1 is the CB prefix, handled before we get here.
                _ => "NOP".into(),
            },
            (_, 4) => format!("CALL {},{}", CONDITIONS[y as usize], hex16(self.next_word())),
            (_, 5) if y & 1 == 0 => {
                let pair = match (y >> 1, index) {
                    (2, Some(name)) => name.to_string(),
                    (code, _) => STACK_PAIRS[code as usize].to_string(),
                };
                format!("PUSH {pair}")
            }
            (_, 5) => format!("CALL {}", hex16(self.next_word())),
            (_, 6) => format!("{}{}", ALU[y as usize], hex8(self.next_byte())),
            (_, _) => format!("RST {}", hex8(y << 3)),
        }
    }

    fn decode_cb(&mut self, index: Option<&str>) -> String {
        // Under a prefix the displacement precedes the opcode byte.
        let displacement = index.map(|_| self.next_byte() as i8);
        let opcode = self.next_byte();
        let y = (opcode >> 3) & 7;
        let z = opcode & 7;

        let target = match (index, displacement) {
            (Some(name), Some(d)) => format!("({name}{d:+})"),
            _ => REG8[z as usize].to_string(),
        };

        // The undocumented indexed forms also store into a register.
        let copy = match (index, z) {
            (Some(_), 6) | (None, _) => String::new(),
            (Some(_), _) => format!(",{}", REG8[z as usize]),
        };

        match opcode >> 6 {
            0 => format!("{} {target}{copy}", ROTATES[y as usize]),
            1 => format!("BIT {y},{target}"),
            2 => format!("RES {y},{target}{copy}"),
            _ => format!("SET {y},{target}{copy}"),
        }
    }

    fn decode_ed(&mut self) -> String {
        let opcode = self.next_byte();
        let y = (opcode >> 3) & 7;
        let z = opcode & 7;
        let pair = (opcode >> 4) & 3;

        match opcode {
            0x40..=0x7F => match z {
                0 if y == 6 => "IN F,(C)".into(),
                0 => format!("IN {},(C)", REG8[y as usize]),
                1 if y == 6 => "OUT (C),0".into(),
                1 => format!("OUT (C),{}", REG8[y as usize]),
                2 if opcode & 0x08 == 0 => format!("SBC HL,{}", PAIRS[pair as usize]),
                2 => format!("ADC HL,{}", PAIRS[pair as usize]),
                3 if opcode & 0x08 == 0 => {
                    format!("LD ({}),{}", hex16(self.next_word()), PAIRS[pair as usize])
                }
                3 => format!("LD {},({})", PAIRS[pair as usize], hex16(self.next_word())),
                4 => "NEG".into(),
                5 if opcode == 0x4D => "RETI".into(),
                5 => "RETN".into(),
                6 => format!("IM {}", [0, 0, 1, 2][(y & 3) as usize]),
                _ => match opcode {
                    0x47 => "LD I,A".into(),
                    0x4F => "LD R,A".into(),
                    0x57 => "LD A,I".into(),
                    0x5F => "LD A,R".into(),
                    0x67 => "RRD".into(),
                    0x6F => "RLD".into(),
                    _ => "NOP".into(),
                },
            },
            0xA0..=0xBB if z < 4 => {
                BLOCK[((opcode >> 3) & 3) as usize][z as usize].to_string()
            }
            // Undefined opcodes in the ED page behave as two NOPs.
            _ => format!("DB 0EDh,{}", hex8(opcode)),
        }
    }

    /// The absolute target of a relative jump, so the text re-assembles.
    fn relative_target(&mut self) -> String {
        let displacement = self.next_byte() as i8 as i16;
        let next = self
            .address
            .wrapping_add(self.bytes.len() as u16)
            .wrapping_add_signed(displacement);
        hex16(next)
    }
}

/// Hex in the assembler's own notation: a trailing `h`, with a leading zero
/// when the value would otherwise start with a letter.
pub fn hex8(value: u8) -> String {
    prefix_zero(format!("{value:02X}h"))
}

pub fn hex16(value: u16) -> String {
    prefix_zero(format!("{value:04X}h"))
}

fn prefix_zero(text: String) -> String {
    if text.starts_with(|c: char| c.is_ascii_alphabetic()) {
        format!("0{text}")
    } else {
        text
    }
}
