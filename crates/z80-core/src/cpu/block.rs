//! The block transfer, search and I/O instructions (`LDIR`, `CPIR`, `OTIR`
//! and friends).
//!
//! All eight base forms share a body; the `R` variants simply rewind PC by
//! two bytes so the instruction is fetched again, which is exactly what the
//! hardware does and keeps interrupt latency correct.

use super::Cpu;
use crate::bus::Bus;
use crate::flags::{C, F3, F5, H, N, PV, S, Z};

impl Cpu {
    /// Returns the extra cycles beyond the table cost (5 when repeating).
    pub(super) fn execute_block<B: Bus>(&mut self, bus: &mut B, opcode: u8) -> u32 {
        let decrement = opcode & 0x08 != 0;
        let repeats = opcode & 0x10 != 0;

        let repeated = match opcode & 7 {
            0 => self.block_transfer(bus, decrement),
            1 => self.block_compare(bus, decrement),
            2 => self.block_input(bus, decrement),
            _ => self.block_output(bus, decrement),
        };

        if repeats && repeated {
            // Rewind over the ED prefix and the opcode so the instruction
            // runs again, leaving a window for interrupts between iterations.
            self.regs.pc = self.regs.pc.wrapping_sub(2);
            self.regs.wz = self.regs.pc.wrapping_add(1);
            5
        } else {
            0
        }
    }

    /// `LDI` / `LDD`. Returns true when BC is still non-zero.
    fn block_transfer<B: Bus>(&mut self, bus: &mut B, decrement: bool) -> bool {
        let hl = self.regs.hl();
        let de = self.regs.de();
        let value = bus.read(hl);
        bus.write(de, value);

        let delta: i16 = if decrement { -1 } else { 1 };
        self.regs.set_hl(hl.wrapping_add_signed(delta));
        self.regs.set_de(de.wrapping_add_signed(delta));
        let bc = self.regs.bc().wrapping_sub(1);
        self.regs.set_bc(bc);

        // Flags 3 and 5 come from A plus the byte just moved, a quirk the
        // conformance suites check closely.
        let marker = self.regs.a.wrapping_add(value);
        self.regs.f = (self.regs.f & (S | Z | C))
            | (marker & F3)
            | if marker & 0x02 != 0 { F5 } else { 0 }
            | if bc != 0 { PV } else { 0 };

        bc != 0
    }

    /// `CPI` / `CPD`. Returns true when BC is non-zero and no match was found.
    fn block_compare<B: Bus>(&mut self, bus: &mut B, decrement: bool) -> bool {
        let hl = self.regs.hl();
        let value = bus.read(hl);
        let a = self.regs.a;
        let result = a.wrapping_sub(value);
        let half_borrow = (a & 0x0F) < (value & 0x0F);

        let delta: i16 = if decrement { -1 } else { 1 };
        self.regs.set_hl(hl.wrapping_add_signed(delta));
        let bc = self.regs.bc().wrapping_sub(1);
        self.regs.set_bc(bc);
        self.regs.wz = self.regs.wz.wrapping_add_signed(delta);

        // The undocumented flags use the result with the half-borrow
        // subtracted back out.
        let marker = result.wrapping_sub(u8::from(half_borrow));
        self.regs.f = (self.regs.f & C)
            | N
            | (result & S)
            | if result == 0 { Z } else { 0 }
            | if half_borrow { H } else { 0 }
            | (marker & F3)
            | if marker & 0x02 != 0 { F5 } else { 0 }
            | if bc != 0 { PV } else { 0 };

        bc != 0 && result != 0
    }

    /// `INI` / `IND`. Returns true when B is still non-zero.
    fn block_input<B: Bus>(&mut self, bus: &mut B, decrement: bool) -> bool {
        let port = self.regs.bc();
        let data = bus.input(port);
        let hl = self.regs.hl();
        bus.write(hl, data);

        let delta: i16 = if decrement { -1 } else { 1 };
        self.regs.wz = port.wrapping_add_signed(delta);
        self.regs.set_hl(hl.wrapping_add_signed(delta));
        self.regs.b = self.regs.b.wrapping_sub(1);

        let addend = self.regs.c.wrapping_add_signed(if decrement { -1 } else { 1 });
        let counter = self.regs.b;
        self.block_io_flags(data, addend, counter);

        counter != 0
    }

    /// `OUTI` / `OUTD`. Returns true when B is still non-zero.
    fn block_output<B: Bus>(&mut self, bus: &mut B, decrement: bool) -> bool {
        let hl = self.regs.hl();
        let data = bus.read(hl);

        // B is decremented before the port write, so the port address that
        // reaches the bus already carries the new count.
        self.regs.b = self.regs.b.wrapping_sub(1);
        let port = self.regs.bc();
        bus.output(port, data);

        let delta: i16 = if decrement { -1 } else { 1 };
        self.regs.set_hl(hl.wrapping_add_signed(delta));
        self.regs.wz = port.wrapping_add_signed(delta);

        let counter = self.regs.b;
        let addend = self.regs.l;
        self.block_io_flags(data, addend, counter);

        counter != 0
    }
}
