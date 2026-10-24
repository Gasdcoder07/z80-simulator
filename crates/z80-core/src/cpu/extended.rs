//! `ED`-prefixed instructions: 16-bit arithmetic with carry, port I/O
//! through BC, the interrupt-mode and I/R register transfers, the nibble
//! rotates, and the block instruction group.

use super::{Cpu, InterruptMode, ED_CYCLES};
use crate::bus::Bus;
use crate::flags::{self, C, H, N, PV};

impl Cpu {
    pub(super) fn execute_ed<B: Bus>(&mut self, bus: &mut B, prefix_cycles: u32) -> u32 {
        let opcode = self.fetch_opcode(bus);
        let mut extra = prefix_cycles;

        match opcode {
            0x40..=0x7F => {
                let group = (opcode >> 3) & 7;
                let pair = (opcode >> 4) & 3;
                match opcode & 7 {
                    // IN r,(C). Group 6 (ED 70) discards the result but
                    // still performs the read and sets the flags.
                    0 => {
                        let port = self.regs.bc();
                        let value = bus.input(port);
                        self.regs.wz = port.wrapping_add(1);
                        if group != 6 {
                            self.set_reg(group, value);
                        }
                        self.regs.f = self.carry() | flags::sz53p(value);
                    }
                    // OUT (C),r. Group 6 (ED 71) writes zero on NMOS parts.
                    1 => {
                        let port = self.regs.bc();
                        let value = if group == 6 { 0 } else { self.reg(group) };
                        bus.output(port, value);
                        self.regs.wz = port.wrapping_add(1);
                    }
                    // SBC HL,rr (even groups) and ADC HL,rr (odd groups).
                    2 => {
                        let left = self.regs.hl();
                        let right = self.pair(pair, super::Index::Hl);
                        self.regs.wz = left.wrapping_add(1);
                        let carry = self.carry();
                        let (result, f) = if opcode & 0x08 == 0 {
                            flags::sbc16(left, right, carry)
                        } else {
                            flags::adc16(left, right, carry)
                        };
                        self.regs.set_hl(result);
                        self.regs.f = f;
                    }
                    // LD (nn),rr and LD rr,(nn).
                    3 => {
                        let addr = self.fetch_word(bus);
                        if opcode & 0x08 == 0 {
                            let value = self.pair(pair, super::Index::Hl);
                            self.write_word(bus, addr, value);
                        } else {
                            let value = self.read_word(bus, addr);
                            self.set_pair(pair, super::Index::Hl, value);
                        }
                        self.regs.wz = addr.wrapping_add(1);
                    }
                    // NEG, in all eight of its encodings.
                    4 => {
                        let (a, f) = flags::neg(self.regs.a);
                        self.regs.a = a;
                        self.regs.f = f;
                    }
                    // RETN and RETI both restore IFF1 from IFF2; only the
                    // bus signalling differs, which a simulator cannot see.
                    5 => {
                        let target = self.pop_word(bus);
                        self.regs.pc = target;
                        self.regs.wz = target;
                        self.iff1 = self.iff2;
                    }
                    // IM n. Two of the eight encodings select the
                    // undocumented "mode 0/1", which behaves as mode 0.
                    6 => {
                        self.interrupt_mode = match (opcode >> 3) & 3 {
                            0 | 1 => InterruptMode::Mode0,
                            2 => InterruptMode::Mode1,
                            _ => InterruptMode::Mode2,
                        };
                    }
                    _ => extra += self.execute_ed_column7(bus, opcode),
                }
            }

            0xA0..=0xBB if opcode & 7 < 4 => {
                extra += self.execute_block(bus, opcode);
            }

            // Everything else in the ED page is undefined and acts as a NOP.
            _ => {}
        }

        ED_CYCLES[opcode as usize] as u32 + extra
    }

    /// `LD I,A`, `LD R,A`, `LD A,I`, `LD A,R`, `RRD` and `RLD`.
    fn execute_ed_column7<B: Bus>(&mut self, bus: &mut B, opcode: u8) -> u32 {
        match opcode {
            0x47 => self.regs.i = self.regs.a,
            0x4F => self.regs.r = self.regs.a,
            0x57 | 0x5F => {
                let value = if opcode == 0x57 { self.regs.i } else { self.regs.r };
                self.regs.a = value;
                // P/V reports the interrupt enable state rather than parity.
                self.regs.f = self.carry()
                    | flags::sz53(value)
                    | if self.iff2 { PV } else { 0 };
            }
            // RRD / RLD rotate one BCD digit between A and (HL).
            0x67 | 0x6F => {
                let addr = self.regs.hl();
                let memory = bus.read(addr);
                let (new_memory, new_a) = if opcode == 0x67 {
                    (
                        (memory >> 4) | (self.regs.a << 4),
                        (self.regs.a & 0xF0) | (memory & 0x0F),
                    )
                } else {
                    (
                        (memory << 4) | (self.regs.a & 0x0F),
                        (self.regs.a & 0xF0) | (memory >> 4),
                    )
                };
                bus.write(addr, new_memory);
                self.regs.a = new_a;
                self.regs.wz = addr.wrapping_add(1);
                self.regs.f = self.carry() | flags::sz53p(new_a);
            }
            // ED 77 and ED 7F are undefined.
            _ => {}
        }
        0
    }

    /// Shared flag tail for `INI`/`IND`/`OUTI`/`OUTD`, whose flags depend on
    /// a byte that never appears in any register.
    #[inline]
    pub(super) fn block_io_flags(&mut self, data: u8, addend: u8, counter: u8) {
        let k = data as u16 + addend as u16;
        self.regs.f = flags::sz53(counter)
            | if data & 0x80 != 0 { N } else { 0 }
            | if k > 0xFF { H | C } else { 0 }
            | flags::PARITY[((k as u8 & 7) ^ counter) as usize];
    }
}
