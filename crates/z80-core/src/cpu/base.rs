//! Decoding and execution of unprefixed opcodes, plus prefix dispatch.

use super::timing::{BASE_CYCLES, INDEX_DISPLACEMENT_CYCLES, PREFIX_CYCLES};
use super::{Cpu, Index};
use crate::bus::Bus;
use crate::flags::{self, C, F3F5, PV, S, Z};

impl Cpu {
    /// Fetch and run one instruction, absorbing any `DD`/`FD` prefixes first.
    pub(super) fn execute<B: Bus>(&mut self, bus: &mut B) -> u32 {
        let mut index = Index::Hl;
        let mut prefix_cycles = 0;
        loop {
            let opcode = self.fetch_opcode(bus);
            match opcode {
                // Only the last of a run of index prefixes has any effect,
                // but each one is still fetched and still bumps R.
                0xDD => {
                    index = Index::Ix;
                    prefix_cycles += PREFIX_CYCLES;
                }
                0xFD => {
                    index = Index::Iy;
                    prefix_cycles += PREFIX_CYCLES;
                }
                _ => return self.execute_opcode(bus, opcode, index, prefix_cycles),
            }
        }
    }

    pub(super) fn execute_opcode<B: Bus>(
        &mut self,
        bus: &mut B,
        opcode: u8,
        index: Index,
        prefix_cycles: u32,
    ) -> u32 {
        if opcode == 0xCB {
            return self.execute_cb(bus, index, prefix_cycles + PREFIX_CYCLES);
        }
        if opcode == 0xED {
            return self.execute_ed(bus, prefix_cycles + PREFIX_CYCLES);
        }

        let mut extra = prefix_cycles;

        match opcode {
            // ---- 8-bit loads between registers, and HALT ----------------
            0x76 => {
                // Park the PC on the HALT opcode so the debugger shows the
                // instruction the CPU is actually sitting on.
                self.regs.pc = self.regs.pc.wrapping_sub(1);
                self.halted = true;
            }
            0x40..=0x7F => {
                let dst = (opcode >> 3) & 7;
                let src = opcode & 7;
                if dst == 6 {
                    let addr = self.operand_addr(bus, index);
                    extra += self.index_cost(index);
                    // When the instruction reaches memory through IX/IY, the
                    // other operand stays plain H or L.
                    let value = self.reg(src);
                    bus.write(addr, value);
                } else if src == 6 {
                    let addr = self.operand_addr(bus, index);
                    extra += self.index_cost(index);
                    let value = bus.read(addr);
                    self.set_reg(dst, value);
                } else if self.half_index_blocked(index) {
                    return self.undocumented_nop(opcode, prefix_cycles);
                } else {
                    let value = self.reg_indexed(src, index);
                    self.set_reg_indexed(dst, index, value);
                }
            }

            // ---- accumulator arithmetic and logic -----------------------
            0x80..=0xBF => {
                let code = opcode & 7;
                let operand = if code == 6 {
                    let addr = self.operand_addr(bus, index);
                    extra += self.index_cost(index);
                    bus.read(addr)
                } else if self.half_index_blocked(index) && (code == 4 || code == 5) {
                    return self.undocumented_nop(opcode, prefix_cycles);
                } else {
                    self.reg_indexed(code, index)
                };
                self.alu((opcode >> 3) & 7, operand);
            }
            0xC6 | 0xCE | 0xD6 | 0xDE | 0xE6 | 0xEE | 0xF6 | 0xFE => {
                let operand = self.fetch(bus);
                self.alu((opcode >> 3) & 7, operand);
            }

            // ---- 8-bit immediate load and INC/DEC -----------------------
            0x06 | 0x0E | 0x16 | 0x1E | 0x26 | 0x2E | 0x36 | 0x3E => {
                let code = (opcode >> 3) & 7;
                if code == 6 {
                    let addr = self.operand_addr(bus, index);
                    let value = self.fetch(bus);
                    bus.write(addr, value);
                    // LD (IX+d),n overlaps its displacement and immediate
                    // fetches, so it costs 19 rather than the usual 22.
                    if index.is_indexed() {
                        extra += INDEX_DISPLACEMENT_CYCLES - 3;
                    }
                } else if self.half_index_blocked(index) && (code == 4 || code == 5) {
                    self.fetch(bus);
                    return self.undocumented_nop(opcode, prefix_cycles);
                } else {
                    let value = self.fetch(bus);
                    self.set_reg_indexed(code, index, value);
                }
            }
            0x04 | 0x0C | 0x14 | 0x1C | 0x24 | 0x2C | 0x34 | 0x3C => {
                extra += self.inc_dec_operand(bus, opcode, index, true);
            }
            0x05 | 0x0D | 0x15 | 0x1D | 0x25 | 0x2D | 0x35 | 0x3D => {
                extra += self.inc_dec_operand(bus, opcode, index, false);
            }

            // ---- 16-bit loads and arithmetic ----------------------------
            0x01 | 0x11 | 0x21 | 0x31 => {
                let value = self.fetch_word(bus);
                self.set_pair((opcode >> 4) & 3, index, value);
            }
            0x03 | 0x13 | 0x23 | 0x33 => {
                let code = (opcode >> 4) & 3;
                let value = self.pair(code, index).wrapping_add(1);
                self.set_pair(code, index, value);
            }
            0x0B | 0x1B | 0x2B | 0x3B => {
                let code = (opcode >> 4) & 3;
                let value = self.pair(code, index).wrapping_sub(1);
                self.set_pair(code, index, value);
            }
            0x09 | 0x19 | 0x29 | 0x39 => {
                let left = index.get(&self.regs);
                let right = self.pair((opcode >> 4) & 3, index);
                self.regs.wz = left.wrapping_add(1);
                let (result, f) = flags::add16(left, right, self.regs.f);
                index.set(&mut self.regs, result);
                self.regs.f = f;
            }

            // ---- loads through a memory address -------------------------
            0x02 | 0x12 => {
                let addr = if opcode == 0x02 { self.regs.bc() } else { self.regs.de() };
                bus.write(addr, self.regs.a);
                self.set_wz_after_write(addr);
            }
            0x0A | 0x1A => {
                let addr = if opcode == 0x0A { self.regs.bc() } else { self.regs.de() };
                self.regs.a = bus.read(addr);
                self.regs.wz = addr.wrapping_add(1);
            }
            0x22 => {
                let addr = self.fetch_word(bus);
                let value = index.get(&self.regs);
                self.write_word(bus, addr, value);
                self.regs.wz = addr.wrapping_add(1);
            }
            0x2A => {
                let addr = self.fetch_word(bus);
                let value = self.read_word(bus, addr);
                index.set(&mut self.regs, value);
                self.regs.wz = addr.wrapping_add(1);
            }
            0x32 => {
                let addr = self.fetch_word(bus);
                bus.write(addr, self.regs.a);
                self.set_wz_after_write(addr);
            }
            0x3A => {
                let addr = self.fetch_word(bus);
                self.regs.a = bus.read(addr);
                self.regs.wz = addr.wrapping_add(1);
            }

            // ---- accumulator rotates and adjustments --------------------
            0x07 | 0x0F | 0x17 | 0x1F => self.rotate_accumulator(opcode),
            0x27 => {
                let (a, f) = flags::daa(self.regs.a, self.regs.f);
                self.regs.a = a;
                self.regs.f = f;
            }
            0x2F => {
                let (a, f) = flags::cpl(self.regs.a, self.regs.f);
                self.regs.a = a;
                self.regs.f = f;
            }
            0x37 => self.regs.f = flags::scf(self.regs.a, self.regs.f),
            0x3F => self.regs.f = flags::ccf(self.regs.a, self.regs.f),

            // ---- exchanges ----------------------------------------------
            0x08 => self.regs.exchange_af(),
            0xD9 => self.regs.exchange_bc_de_hl(),
            // EX DE,HL always means the real DE and HL, even under a prefix.
            0xEB => self.regs.exchange_de_hl(),
            0xE3 => {
                let sp = self.regs.sp;
                let from_stack = self.read_word(bus, sp);
                let value = index.get(&self.regs);
                self.write_word(bus, sp, value);
                index.set(&mut self.regs, from_stack);
                self.regs.wz = from_stack;
            }

            // ---- relative jumps ------------------------------------------
            0x10 => {
                let displacement = self.fetch(bus) as i8 as i16;
                self.regs.b = self.regs.b.wrapping_sub(1);
                if self.regs.b != 0 {
                    self.jump_relative(displacement);
                    extra += 5;
                }
            }
            0x18 => {
                let displacement = self.fetch(bus) as i8 as i16;
                self.jump_relative(displacement);
            }
            0x20 | 0x28 | 0x30 | 0x38 => {
                let displacement = self.fetch(bus) as i8 as i16;
                if self.condition((opcode >> 3) & 3) {
                    self.jump_relative(displacement);
                    extra += 5;
                }
            }

            // ---- absolute jumps, calls and returns -----------------------
            0xC3 => {
                let target = self.fetch_word(bus);
                self.regs.pc = target;
                self.regs.wz = target;
            }
            0xC2 | 0xCA | 0xD2 | 0xDA | 0xE2 | 0xEA | 0xF2 | 0xFA => {
                let target = self.fetch_word(bus);
                // WZ latches the destination whether or not the jump happens.
                self.regs.wz = target;
                if self.condition((opcode >> 3) & 7) {
                    self.regs.pc = target;
                }
            }
            0xE9 => self.regs.pc = index.get(&self.regs),
            0xCD => {
                let target = self.fetch_word(bus);
                self.call(bus, target);
            }
            0xC4 | 0xCC | 0xD4 | 0xDC | 0xE4 | 0xEC | 0xF4 | 0xFC => {
                let target = self.fetch_word(bus);
                self.regs.wz = target;
                if self.condition((opcode >> 3) & 7) {
                    self.call(bus, target);
                    extra += 7;
                }
            }
            0xC9 => {
                let target = self.pop_word(bus);
                self.regs.pc = target;
                self.regs.wz = target;
            }
            0xC0 | 0xC8 | 0xD0 | 0xD8 | 0xE0 | 0xE8 | 0xF0 | 0xF8 => {
                if self.condition((opcode >> 3) & 7) {
                    let target = self.pop_word(bus);
                    self.regs.pc = target;
                    self.regs.wz = target;
                    extra += 6;
                }
            }
            0xC7 | 0xCF | 0xD7 | 0xDF | 0xE7 | 0xEF | 0xF7 | 0xFF => {
                let target = (opcode & 0x38) as u16;
                self.call(bus, target);
            }

            // ---- stack ---------------------------------------------------
            0xC5 | 0xD5 | 0xE5 | 0xF5 => {
                let value = self.push_pop_source((opcode >> 4) & 3, index);
                self.push_word(bus, value);
            }
            0xC1 | 0xD1 | 0xE1 | 0xF1 => {
                let value = self.pop_word(bus);
                self.store_push_pop((opcode >> 4) & 3, index, value);
            }
            0xF9 => self.regs.sp = index.get(&self.regs),

            // ---- port I/O -------------------------------------------------
            0xDB => {
                let low = self.fetch(bus);
                let port = u16::from_be_bytes([self.regs.a, low]);
                self.regs.a = bus.input(port);
                self.regs.wz = port.wrapping_add(1);
            }
            0xD3 => {
                let low = self.fetch(bus);
                let port = u16::from_be_bytes([self.regs.a, low]);
                bus.output(port, self.regs.a);
                self.set_wz_after_write(port);
            }

            // ---- interrupt control ----------------------------------------
            0xF3 => {
                self.iff1 = false;
                self.iff2 = false;
            }
            0xFB => {
                self.iff1 = true;
                self.iff2 = true;
                self.ei_pending = true;
            }

            0x00 => {}

            // Every opcode is accounted for above; the remaining patterns in
            // 0x00-0x3F are plain NOPs on real silicon.
            _ => {}
        }

        BASE_CYCLES[opcode as usize] as u32 + extra
    }

    /// Extra cycles when an `(HL)` operand was redirected through IX or IY.
    #[inline]
    fn index_cost(&self, index: Index) -> u32 {
        if index.is_indexed() {
            INDEX_DISPLACEMENT_CYCLES
        } else {
            0
        }
    }

    /// True when the instruction would touch IXH/IXL/IYH/IYL but the user
    /// has turned undocumented instructions off.
    #[inline]
    fn half_index_blocked(&self, index: Index) -> bool {
        index.is_indexed() && !self.allow_undocumented
    }

    /// Cost of a prefixed instruction that was suppressed: the prefix still
    /// executed as a NOP, and so did the opcode behind it.
    #[inline]
    fn undocumented_nop(&self, opcode: u8, prefix_cycles: u32) -> u32 {
        BASE_CYCLES[opcode as usize] as u32 + prefix_cycles
    }

    fn inc_dec_operand<B: Bus>(
        &mut self,
        bus: &mut B,
        opcode: u8,
        index: Index,
        increment: bool,
    ) -> u32 {
        let code = (opcode >> 3) & 7;
        if code == 6 {
            let addr = self.operand_addr(bus, index);
            let value = bus.read(addr);
            let (result, f) = if increment {
                flags::inc8(value, self.regs.f)
            } else {
                flags::dec8(value, self.regs.f)
            };
            bus.write(addr, result);
            self.regs.f = f;
            self.index_cost(index)
        } else {
            if self.half_index_blocked(index) && (code == 4 || code == 5) {
                return 0;
            }
            let value = self.reg_indexed(code, index);
            let (result, f) = if increment {
                flags::inc8(value, self.regs.f)
            } else {
                flags::dec8(value, self.regs.f)
            };
            self.set_reg_indexed(code, index, result);
            self.regs.f = f;
            0
        }
    }

    /// The eight accumulator ALU operations, selected by the middle three
    /// bits of the opcode.
    fn alu(&mut self, op: u8, operand: u8) {
        let a = self.regs.a;
        let carry = self.carry();
        let (result, f) = match op {
            0 => flags::add8(a, operand, 0),
            1 => flags::add8(a, operand, carry),
            2 => flags::sub8(a, operand, 0),
            3 => flags::sub8(a, operand, carry),
            4 => flags::and8(a, operand),
            5 => flags::xor8(a, operand),
            6 => flags::or8(a, operand),
            _ => {
                // CP discards the result but keeps the flags.
                self.regs.f = flags::cp8(a, operand);
                return;
            }
        };
        self.regs.a = result;
        self.regs.f = f;
    }

    /// `RLCA`, `RRCA`, `RLA`, `RRA`: these keep S, Z and P/V, unlike their
    /// `CB`-prefixed cousins.
    fn rotate_accumulator(&mut self, opcode: u8) {
        let a = self.regs.a;
        let carry_in = self.carry();
        let (result, carry_out) = match (opcode >> 3) & 3 {
            0 => (a.rotate_left(1), a & 0x80 != 0),
            1 => (a.rotate_right(1), a & 0x01 != 0),
            2 => ((a << 1) | carry_in, a & 0x80 != 0),
            _ => ((a >> 1) | (carry_in << 7), a & 0x01 != 0),
        };
        self.regs.a = result;
        self.regs.f = (self.regs.f & (S | Z | PV))
            | (result & F3F5)
            | if carry_out { C } else { 0 };
    }

    #[inline]
    fn jump_relative(&mut self, displacement: i16) {
        let target = self.regs.pc.wrapping_add_signed(displacement);
        self.regs.pc = target;
        self.regs.wz = target;
    }

    #[inline]
    fn call<B: Bus>(&mut self, bus: &mut B, target: u16) {
        let return_addr = self.regs.pc;
        self.push_word(bus, return_addr);
        self.regs.pc = target;
        self.regs.wz = target;
    }

    /// `PUSH`/`POP` use AF where the other 16-bit groups use SP.
    #[inline]
    fn push_pop_source(&self, code: u8, index: Index) -> u16 {
        if code == 3 {
            self.regs.af()
        } else {
            self.pair(code, index)
        }
    }

    #[inline]
    fn store_push_pop(&mut self, code: u8, index: Index, value: u16) {
        if code == 3 {
            self.regs.set_af(value);
        } else {
            self.set_pair(code, index, value);
        }
    }

    /// After writing a byte to `addr`, WZ holds the next address in its low
    /// half and the accumulator in its high half.
    #[inline]
    pub(super) fn set_wz_after_write(&mut self, addr: u16) {
        self.regs.wz = u16::from_be_bytes([self.regs.a, (addr as u8).wrapping_add(1)]);
    }
}
