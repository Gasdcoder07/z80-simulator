//! The CPU itself: fetch, decode, execute, and interrupt acceptance.

mod base;
mod bit_ops;
mod block;
mod extended;
mod timing;

use crate::bus::Bus;
use crate::flags;
use crate::registers::Registers;

pub(crate) use timing::ED_CYCLES;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Which 16-bit register an instruction's `(HL)` operand actually refers to.
/// `DD` and `FD` prefixes swap in IX and IY respectively.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Index {
    Hl,
    Ix,
    Iy,
}

impl Index {
    #[inline]
    fn get(self, regs: &Registers) -> u16 {
        match self {
            Index::Hl => regs.hl(),
            Index::Ix => regs.ix,
            Index::Iy => regs.iy,
        }
    }

    #[inline]
    fn set(self, regs: &mut Registers, value: u16) {
        match self {
            Index::Hl => regs.set_hl(value),
            Index::Ix => regs.ix = value,
            Index::Iy => regs.iy = value,
        }
    }

    #[inline]
    fn is_indexed(self) -> bool {
        !matches!(self, Index::Hl)
    }
}

/// Interrupt response mode selected by `IM 0`, `IM 1` and `IM 2`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum InterruptMode {
    /// Execute the instruction the device places on the data bus.
    #[default]
    Mode0,
    /// Restart at 0038H.
    Mode1,
    /// Vector table indexed by I and the byte from the device.
    Mode2,
}

impl InterruptMode {
    pub fn from_code(code: u8) -> Self {
        match code {
            1 => InterruptMode::Mode1,
            2 => InterruptMode::Mode2,
            _ => InterruptMode::Mode0,
        }
    }

    pub fn code(self) -> u8 {
        match self {
            InterruptMode::Mode0 => 0,
            InterruptMode::Mode1 => 1,
            InterruptMode::Mode2 => 2,
        }
    }
}

/// What the CPU did during one call to [`Cpu::step`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct StepResult {
    /// Clock cycles consumed.
    pub tstates: u32,
    /// True when the step only serviced the HALT state rather than running
    /// a real instruction.
    pub halted: bool,
    /// Set when this step accepted a maskable or non-maskable interrupt.
    pub interrupt_taken: bool,
}

/// A Zilog Z80 processor.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Cpu {
    pub regs: Registers,
    /// Master interrupt enable.
    pub iff1: bool,
    /// Shadow copy of `iff1`, saved across NMI and restored by `RETN`.
    pub iff2: bool,
    pub interrupt_mode: InterruptMode,
    /// True while stopped on a `HALT` waiting for an interrupt.
    pub halted: bool,
    /// `EI` only takes effect after the following instruction, so that
    /// `EI; RET` cannot be interrupted before the return.
    pub ei_pending: bool,
    /// When false, undocumented opcodes are treated as NOPs rather than
    /// executed. Mirrors the "Enable Unofficial Instructions" option.
    pub allow_undocumented: bool,

    nmi_pending: bool,
    int_line: bool,
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new(false)
    }
}

impl Cpu {
    /// `fill_ff` selects the power-on register contents: all ones like real
    /// hardware, or all zeros.
    pub fn new(fill_ff: bool) -> Self {
        Self {
            regs: Registers::new(fill_ff),
            iff1: false,
            iff2: false,
            interrupt_mode: InterruptMode::Mode0,
            halted: false,
            ei_pending: false,
            allow_undocumented: true,
            nmi_pending: false,
            int_line: false,
        }
    }

    /// Pull RESET. Clears PC, I, R and the interrupt state; leaves the rest
    /// of the register file alone, as the hardware does.
    pub fn reset(&mut self) {
        self.regs.pc = 0;
        self.regs.i = 0;
        self.regs.r = 0;
        self.regs.wz = 0;
        self.iff1 = false;
        self.iff2 = false;
        self.interrupt_mode = InterruptMode::Mode0;
        self.halted = false;
        self.ei_pending = false;
        self.nmi_pending = false;
        self.int_line = false;
    }

    /// Hold or release the maskable interrupt line. It stays asserted until
    /// cleared, matching a real device that waits to be acknowledged.
    pub fn set_int_line(&mut self, asserted: bool) {
        self.int_line = asserted;
    }

    pub fn int_line(&self) -> bool {
        self.int_line
    }

    /// Queue an edge-triggered non-maskable interrupt.
    pub fn request_nmi(&mut self) {
        self.nmi_pending = true;
    }

    /// Execute one instruction, or accept one pending interrupt.
    pub fn step<B: Bus>(&mut self, bus: &mut B) -> StepResult {
        let was_ei_pending = self.ei_pending;
        self.ei_pending = false;

        if self.nmi_pending {
            self.nmi_pending = false;
            let tstates = self.accept_nmi(bus);
            bus.tick(tstates);
            return StepResult { tstates, halted: false, interrupt_taken: true };
        }

        // An interrupt is never accepted directly after EI: the enable only
        // becomes visible once the following instruction has run.
        if self.int_line && self.iff1 && !was_ei_pending {
            let tstates = self.accept_int(bus);
            bus.tick(tstates);
            return StepResult { tstates, halted: false, interrupt_taken: true };
        }

        if self.halted {
            // A halted CPU keeps executing internal NOPs until interrupted.
            self.regs.bump_r();
            bus.tick(4);
            return StepResult { tstates: 4, halted: true, interrupt_taken: false };
        }

        let tstates = self.execute(bus);
        bus.tick(tstates);
        StepResult { tstates, halted: self.halted, interrupt_taken: false }
    }

    fn accept_nmi<B: Bus>(&mut self, bus: &mut B) -> u32 {
        self.leave_halt();
        self.regs.bump_r();
        self.iff2 = self.iff1;
        self.iff1 = false;
        let pc = self.regs.pc;
        self.push_word(bus, pc);
        self.regs.pc = 0x0066;
        self.regs.wz = 0x0066;
        11
    }

    fn accept_int<B: Bus>(&mut self, bus: &mut B) -> u32 {
        self.leave_halt();
        self.regs.bump_r();
        self.iff1 = false;
        self.iff2 = false;

        match self.interrupt_mode {
            InterruptMode::Mode0 => {
                // The device supplies an opcode; in practice always an RST.
                let opcode = bus.interrupt_data();
                let pc = self.regs.pc;
                if opcode & 0xC7 == 0xC7 {
                    let target = (opcode & 0x38) as u16;
                    self.push_word(bus, pc);
                    self.regs.pc = target;
                    self.regs.wz = target;
                    13
                } else {
                    // Anything else is executed in place, which costs two
                    // extra cycles on top of the instruction itself.
                    self.execute_opcode(bus, opcode, Index::Hl, 0) + 2
                }
            }
            InterruptMode::Mode1 => {
                let pc = self.regs.pc;
                self.push_word(bus, pc);
                self.regs.pc = 0x0038;
                self.regs.wz = 0x0038;
                13
            }
            InterruptMode::Mode2 => {
                let vector = bus.interrupt_data();
                let table = u16::from_be_bytes([self.regs.i, vector & 0xFE]);
                let pc = self.regs.pc;
                self.push_word(bus, pc);
                let target = self.read_word(bus, table);
                self.regs.pc = target;
                self.regs.wz = target;
                19
            }
        }
    }

    /// Wake from HALT, stepping PC past the HALT opcode the fetch unit keeps
    /// re-reading.
    fn leave_halt(&mut self) {
        if self.halted {
            self.halted = false;
            self.regs.pc = self.regs.pc.wrapping_add(1);
        }
    }

    // ---- fetch and memory helpers -------------------------------------

    #[inline]
    fn fetch_opcode<B: Bus>(&mut self, bus: &mut B) -> u8 {
        self.regs.bump_r();
        self.fetch(bus)
    }

    #[inline]
    fn fetch<B: Bus>(&mut self, bus: &mut B) -> u8 {
        let byte = bus.read(self.regs.pc);
        self.regs.pc = self.regs.pc.wrapping_add(1);
        byte
    }

    #[inline]
    fn fetch_word<B: Bus>(&mut self, bus: &mut B) -> u16 {
        let lo = self.fetch(bus);
        let hi = self.fetch(bus);
        u16::from_be_bytes([hi, lo])
    }

    #[inline]
    fn read_word<B: Bus>(&mut self, bus: &mut B, addr: u16) -> u16 {
        let lo = bus.read(addr);
        let hi = bus.read(addr.wrapping_add(1));
        u16::from_be_bytes([hi, lo])
    }

    #[inline]
    fn write_word<B: Bus>(&mut self, bus: &mut B, addr: u16, value: u16) {
        let [hi, lo] = value.to_be_bytes();
        bus.write(addr, lo);
        bus.write(addr.wrapping_add(1), hi);
    }

    #[inline]
    fn push_word<B: Bus>(&mut self, bus: &mut B, value: u16) {
        let [hi, lo] = value.to_be_bytes();
        self.regs.sp = self.regs.sp.wrapping_sub(1);
        bus.write(self.regs.sp, hi);
        self.regs.sp = self.regs.sp.wrapping_sub(1);
        bus.write(self.regs.sp, lo);
    }

    #[inline]
    fn pop_word<B: Bus>(&mut self, bus: &mut B) -> u16 {
        let lo = bus.read(self.regs.sp);
        self.regs.sp = self.regs.sp.wrapping_add(1);
        let hi = bus.read(self.regs.sp);
        self.regs.sp = self.regs.sp.wrapping_add(1);
        u16::from_be_bytes([hi, lo])
    }

    /// Resolve the address an `(HL)` operand refers to, consuming the
    /// displacement byte when a `DD`/`FD` prefix is active.
    fn operand_addr<B: Bus>(&mut self, bus: &mut B, index: Index) -> u16 {
        match index {
            Index::Hl => self.regs.hl(),
            other => {
                let displacement = self.fetch(bus) as i8 as i16;
                let addr = other.get(&self.regs).wrapping_add_signed(displacement);
                self.regs.wz = addr;
                addr
            }
        }
    }

    // ---- 8-bit register file access -----------------------------------

    /// Read register `code` (0=B .. 7=A) without any index substitution.
    /// Code 6 is invalid here; callers handle memory operands themselves.
    #[inline]
    fn reg(&self, code: u8) -> u8 {
        match code {
            0 => self.regs.b,
            1 => self.regs.c,
            2 => self.regs.d,
            3 => self.regs.e,
            4 => self.regs.h,
            5 => self.regs.l,
            7 => self.regs.a,
            _ => unreachable!("memory operand must be handled by the caller"),
        }
    }

    #[inline]
    fn set_reg(&mut self, code: u8, value: u8) {
        match code {
            0 => self.regs.b = value,
            1 => self.regs.c = value,
            2 => self.regs.d = value,
            3 => self.regs.e = value,
            4 => self.regs.h = value,
            5 => self.regs.l = value,
            7 => self.regs.a = value,
            _ => unreachable!("memory operand must be handled by the caller"),
        }
    }

    /// Like [`Cpu::reg`], but H and L become IXH/IXL or IYH/IYL under a
    /// `DD`/`FD` prefix. Only valid for instructions that do *not* also
    /// address memory through the index register.
    #[inline]
    fn reg_indexed(&self, code: u8, index: Index) -> u8 {
        match (code, index) {
            (4, Index::Ix) => self.regs.ixh(),
            (5, Index::Ix) => self.regs.ixl(),
            (4, Index::Iy) => self.regs.iyh(),
            (5, Index::Iy) => self.regs.iyl(),
            _ => self.reg(code),
        }
    }

    #[inline]
    fn set_reg_indexed(&mut self, code: u8, index: Index, value: u8) {
        match (code, index) {
            (4, Index::Ix) => self.regs.set_ixh(value),
            (5, Index::Ix) => self.regs.set_ixl(value),
            (4, Index::Iy) => self.regs.set_iyh(value),
            (5, Index::Iy) => self.regs.set_iyl(value),
            _ => self.set_reg(code, value),
        }
    }

    /// Read the 16-bit pair `code` selects (0=BC, 1=DE, 2=HL/IX/IY, 3=SP).
    #[inline]
    fn pair(&self, code: u8, index: Index) -> u16 {
        match code {
            0 => self.regs.bc(),
            1 => self.regs.de(),
            2 => index.get(&self.regs),
            _ => self.regs.sp,
        }
    }

    #[inline]
    fn set_pair(&mut self, code: u8, index: Index, value: u16) {
        match code {
            0 => self.regs.set_bc(value),
            1 => self.regs.set_de(value),
            2 => index.set(&mut self.regs, value),
            _ => self.regs.sp = value,
        }
    }

    /// Evaluate one of the eight branch conditions.
    #[inline]
    fn condition(&self, code: u8) -> bool {
        let f = self.regs.f;
        match code {
            0 => f & flags::Z == 0,
            1 => f & flags::Z != 0,
            2 => f & flags::C == 0,
            3 => f & flags::C != 0,
            4 => f & flags::PV == 0,
            5 => f & flags::PV != 0,
            6 => f & flags::S == 0,
            _ => f & flags::S != 0,
        }
    }

    #[inline]
    fn carry(&self) -> u8 {
        self.regs.f & flags::C
    }
}
