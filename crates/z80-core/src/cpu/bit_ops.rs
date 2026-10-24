//! `CB`-prefixed rotates, shifts and bit operations.

use super::timing::{cb_cycles, INDEX_DISPLACEMENT_CYCLES};
use super::{Cpu, Index};
use crate::bus::Bus;
use crate::flags::{self, F3F5, H, PV, S, Z};

impl Cpu {
    pub(super) fn execute_cb<B: Bus>(
        &mut self,
        bus: &mut B,
        index: Index,
        prefix_cycles: u32,
    ) -> u32 {
        // `DD CB d op` reads the displacement before the opcode, and neither
        // of those bytes is an M1 cycle, so R is not bumped again here.
        let (opcode, addr) = if index.is_indexed() {
            let displacement = self.fetch(bus) as i8 as i16;
            let addr = index.get(&self.regs).wrapping_add_signed(displacement);
            self.regs.wz = addr;
            (self.fetch(bus), Some(addr))
        } else {
            let opcode = self.fetch_opcode(bus);
            let code = opcode & 7;
            (opcode, if code == 6 { Some(self.regs.hl()) } else { None })
        };

        let code = opcode & 7;
        let bit = (opcode >> 3) & 7;
        let indexed = index.is_indexed();

        // Undocumented: the indexed forms also copy their result into the
        // register named by the low three bits.
        let writes_register = code != 6;
        if indexed && writes_register && !self.allow_undocumented {
            return cb_cycles(opcode) + prefix_cycles + INDEX_DISPLACEMENT_CYCLES;
        }

        let value = match addr {
            Some(a) => bus.read(a),
            None => self.reg(code),
        };

        match opcode >> 6 {
            // Rotate and shift group.
            0 => {
                let carry_in = self.carry();
                let (result, f) = match bit {
                    0 => flags::rlc(value),
                    1 => flags::rrc(value),
                    2 => flags::rl(value, carry_in),
                    3 => flags::rr(value, carry_in),
                    4 => flags::sla(value),
                    5 => flags::sra(value),
                    6 => {
                        if !self.allow_undocumented {
                            // SLL is the one undocumented shift; skip it
                            // entirely when the option is off.
                            return cb_cycles(opcode)
                                + prefix_cycles
                                + if indexed { INDEX_DISPLACEMENT_CYCLES } else { 0 };
                        }
                        flags::sll(value)
                    }
                    _ => flags::srl(value),
                };
                self.regs.f = f;
                self.store_cb_result(bus, addr, code, result, indexed, writes_register);
            }
            // BIT: tests a bit and writes nothing back.
            1 => {
                let is_set = value & (1 << bit) != 0;
                let mut f = self.carry() | H;
                if !is_set {
                    f |= Z | PV;
                } else if bit == 7 {
                    f |= S;
                }
                // Flags 3 and 5 come from the operand for register forms, and
                // from the high byte of the internal WZ pointer for memory
                // forms - the single most-tested undocumented behaviour.
                f |= if addr.is_some() {
                    (self.regs.wz >> 8) as u8 & F3F5
                } else {
                    value & F3F5
                };
                self.regs.f = f;
            }
            // RES
            2 => {
                let result = value & !(1 << bit);
                self.store_cb_result(bus, addr, code, result, indexed, writes_register);
            }
            // SET
            _ => {
                let result = value | (1 << bit);
                self.store_cb_result(bus, addr, code, result, indexed, writes_register);
            }
        }

        cb_cycles(opcode)
            + prefix_cycles
            + if indexed { INDEX_DISPLACEMENT_CYCLES } else { 0 }
    }

    fn store_cb_result<B: Bus>(
        &mut self,
        bus: &mut B,
        addr: Option<u16>,
        code: u8,
        result: u8,
        indexed: bool,
        writes_register: bool,
    ) {
        match addr {
            Some(a) => {
                bus.write(a, result);
                if indexed && writes_register {
                    self.set_reg(code, result);
                }
            }
            None => self.set_reg(code, result),
        }
    }
}
