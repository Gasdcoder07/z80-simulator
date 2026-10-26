//! The Z80 register file, including the shadow set and the internal WZ latch.

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Complete Z80 register state.
///
/// `wz` is the undocumented internal pointer (commonly called MEMPTR). It is
/// not reachable by any instruction, but `BIT n,(HL)` leaks its high byte into
/// flags 3 and 5, so an accurate core has to maintain it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct Registers {
    pub a: u8,
    pub f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,

    pub a_alt: u8,
    pub f_alt: u8,
    pub b_alt: u8,
    pub c_alt: u8,
    pub d_alt: u8,
    pub e_alt: u8,
    pub h_alt: u8,
    pub l_alt: u8,

    pub ix: u16,
    pub iy: u16,
    pub sp: u16,
    pub pc: u16,

    pub i: u8,
    pub r: u8,

    pub wz: u16,
}

macro_rules! pair {
    ($get:ident, $set:ident, $hi:ident, $lo:ident) => {
        #[inline]
        pub fn $get(&self) -> u16 {
            u16::from_be_bytes([self.$hi, self.$lo])
        }

        #[inline]
        pub fn $set(&mut self, value: u16) {
            let [hi, lo] = value.to_be_bytes();
            self.$hi = hi;
            self.$lo = lo;
        }
    };
}

impl Registers {
    /// Power-on state. Real Z80s come up with everything set, but the
    /// Oshonsoft simulator lets you choose, so both are supported.
    pub fn new(fill_ff: bool) -> Self {
        let mut regs = Self::default();
        if fill_ff {
            regs.fill(0xFF);
        }
        regs.pc = 0;
        regs.wz = 0;
        regs
    }

    fn fill(&mut self, byte: u8) {
        let word = u16::from_be_bytes([byte, byte]);
        *self = Self {
            a: byte,
            f: byte,
            b: byte,
            c: byte,
            d: byte,
            e: byte,
            h: byte,
            l: byte,
            a_alt: byte,
            f_alt: byte,
            b_alt: byte,
            c_alt: byte,
            d_alt: byte,
            e_alt: byte,
            h_alt: byte,
            l_alt: byte,
            ix: word,
            iy: word,
            sp: word,
            pc: 0,
            i: byte,
            r: byte,
            wz: 0,
        };
    }

    pair!(af, set_af, a, f);
    pair!(bc, set_bc, b, c);
    pair!(de, set_de, d, e);
    pair!(hl, set_hl, h, l);

    #[inline]
    pub fn ixh(&self) -> u8 {
        (self.ix >> 8) as u8
    }

    #[inline]
    pub fn ixl(&self) -> u8 {
        self.ix as u8
    }

    #[inline]
    pub fn iyh(&self) -> u8 {
        (self.iy >> 8) as u8
    }

    #[inline]
    pub fn iyl(&self) -> u8 {
        self.iy as u8
    }

    #[inline]
    pub fn set_ixh(&mut self, value: u8) {
        self.ix = (self.ix & 0x00FF) | ((value as u16) << 8);
    }

    #[inline]
    pub fn set_ixl(&mut self, value: u8) {
        self.ix = (self.ix & 0xFF00) | value as u16;
    }

    #[inline]
    pub fn set_iyh(&mut self, value: u8) {
        self.iy = (self.iy & 0x00FF) | ((value as u16) << 8);
    }

    #[inline]
    pub fn set_iyl(&mut self, value: u8) {
        self.iy = (self.iy & 0xFF00) | value as u16;
    }

    /// `EX AF,AF'`
    #[inline]
    pub fn exchange_af(&mut self) {
        core::mem::swap(&mut self.a, &mut self.a_alt);
        core::mem::swap(&mut self.f, &mut self.f_alt);
    }

    /// `EXX`
    #[inline]
    pub fn exchange_bc_de_hl(&mut self) {
        core::mem::swap(&mut self.b, &mut self.b_alt);
        core::mem::swap(&mut self.c, &mut self.c_alt);
        core::mem::swap(&mut self.d, &mut self.d_alt);
        core::mem::swap(&mut self.e, &mut self.e_alt);
        core::mem::swap(&mut self.h, &mut self.h_alt);
        core::mem::swap(&mut self.l, &mut self.l_alt);
    }

    /// `EX DE,HL`
    #[inline]
    pub fn exchange_de_hl(&mut self) {
        core::mem::swap(&mut self.d, &mut self.h);
        core::mem::swap(&mut self.e, &mut self.l);
    }

    /// Bump the low 7 bits of R, leaving bit 7 as the program set it.
    /// One bump happens per M1 (opcode fetch) cycle, so prefixed
    /// instructions bump it more than once.
    #[inline]
    pub fn bump_r(&mut self) {
        self.r = (self.r & 0x80) | (self.r.wrapping_add(1) & 0x7F);
    }
}

/// One addressable slot in the register file, as the CPU view edits them.
///
/// The debugger writes registers one at a time: sending the whole file back
/// would overwrite whatever the CPU changed since the snapshot the window is
/// looking at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum RegisterId {
    A,
    F,
    B,
    C,
    D,
    E,
    H,
    L,
    AAlt,
    FAlt,
    BAlt,
    CAlt,
    DAlt,
    EAlt,
    HAlt,
    LAlt,
    Ix,
    Iy,
    Sp,
    Pc,
    I,
    R,
    Wz,
}

impl RegisterId {
    /// True for the slots that hold a word; the rest are a single byte.
    #[inline]
    pub fn is_wide(self) -> bool {
        matches!(
            self,
            RegisterId::Ix | RegisterId::Iy | RegisterId::Sp | RegisterId::Pc | RegisterId::Wz
        )
    }

    /// Every slot, in the order the register view lays them out.
    pub const ALL: [RegisterId; 23] = [
        RegisterId::A,
        RegisterId::F,
        RegisterId::B,
        RegisterId::C,
        RegisterId::D,
        RegisterId::E,
        RegisterId::H,
        RegisterId::L,
        RegisterId::AAlt,
        RegisterId::FAlt,
        RegisterId::BAlt,
        RegisterId::CAlt,
        RegisterId::DAlt,
        RegisterId::EAlt,
        RegisterId::HAlt,
        RegisterId::LAlt,
        RegisterId::Ix,
        RegisterId::Iy,
        RegisterId::Sp,
        RegisterId::Pc,
        RegisterId::I,
        RegisterId::R,
        RegisterId::Wz,
    ];
}

impl Registers {
    /// Read one slot. Byte slots widen into the low half.
    pub fn get(&self, id: RegisterId) -> u16 {
        use RegisterId::*;
        match id {
            A => self.a as u16,
            F => self.f as u16,
            B => self.b as u16,
            C => self.c as u16,
            D => self.d as u16,
            E => self.e as u16,
            H => self.h as u16,
            L => self.l as u16,
            AAlt => self.a_alt as u16,
            FAlt => self.f_alt as u16,
            BAlt => self.b_alt as u16,
            CAlt => self.c_alt as u16,
            DAlt => self.d_alt as u16,
            EAlt => self.e_alt as u16,
            HAlt => self.h_alt as u16,
            LAlt => self.l_alt as u16,
            Ix => self.ix,
            Iy => self.iy,
            Sp => self.sp,
            Pc => self.pc,
            I => self.i as u16,
            R => self.r as u16,
            Wz => self.wz,
        }
    }

    /// Write one slot. Byte slots take the low half of `value`.
    pub fn set(&mut self, id: RegisterId, value: u16) {
        use RegisterId::*;
        let byte = value as u8;
        match id {
            A => self.a = byte,
            F => self.f = byte,
            B => self.b = byte,
            C => self.c = byte,
            D => self.d = byte,
            E => self.e = byte,
            H => self.h = byte,
            L => self.l = byte,
            AAlt => self.a_alt = byte,
            FAlt => self.f_alt = byte,
            BAlt => self.b_alt = byte,
            CAlt => self.c_alt = byte,
            DAlt => self.d_alt = byte,
            EAlt => self.e_alt = byte,
            HAlt => self.h_alt = byte,
            LAlt => self.l_alt = byte,
            Ix => self.ix = value,
            Iy => self.iy = value,
            Sp => self.sp = value,
            Pc => self.pc = value,
            I => self.i = byte,
            R => self.r = byte,
            Wz => self.wz = value,
        }
    }
}

#[cfg(test)]
mod register_id_tests {
    use super::*;

    #[test]
    fn byte_slots_keep_only_the_low_half() {
        let mut regs = Registers::default();
        regs.set(RegisterId::AAlt, 0x1234);
        assert_eq!(regs.a_alt, 0x34);
        assert_eq!(regs.get(RegisterId::AAlt), 0x34);
    }

    #[test]
    fn wide_slots_keep_the_whole_word() {
        let mut regs = Registers::default();
        regs.set(RegisterId::Ix, 0x1234);
        assert_eq!(regs.ix, 0x1234);
        assert_eq!(regs.get(RegisterId::Ix), 0x1234);
    }

    #[test]
    fn every_slot_round_trips_and_touches_nothing_else() {
        for (index, id) in RegisterId::ALL.into_iter().enumerate() {
            let mut regs = Registers::default();
            let value = if id.is_wide() { 0xBE00 | index as u16 } else { 0x40 + index as u16 };

            regs.set(id, value);
            assert_eq!(regs.get(id), value, "{id:?} did not round trip");

            let mut cleared = regs;
            cleared.set(id, 0);
            assert_eq!(cleared, Registers::default(), "{id:?} spilled into another slot");
        }
    }
}
