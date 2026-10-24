//! The 64K external memory, with the read/write access tracking the memory
//! editors use to colour recently touched cells.

/// Access flags recorded per byte since the last clear.
pub mod access {
    pub const READ: u8 = 0b01;
    pub const WRITE: u8 = 0b10;
}

pub const SIZE: usize = 0x10000;

pub struct Memory {
    bytes: Box<[u8; SIZE]>,
    access: Box<[u8; SIZE]>,
    /// Whether to record accesses at all. Tracking costs a write per access,
    /// which is worth skipping at the Ultimate simulation rate.
    pub tracking: bool,
}

impl Default for Memory {
    fn default() -> Self {
        Self::new()
    }
}

impl Memory {
    pub fn new() -> Self {
        Self {
            bytes: Box::new([0; SIZE]),
            access: Box::new([0; SIZE]),
            tracking: true,
        }
    }

    /// Fill memory with `NOP`, which is what "Clear Memory" does.
    pub fn clear(&mut self) {
        self.bytes.fill(0x00);
        self.access.fill(0);
    }

    /// Read without recording an access, for the debugger's own use.
    #[inline]
    pub fn peek(&self, address: u16) -> u8 {
        self.bytes[address as usize]
    }

    /// Write without recording an access.
    #[inline]
    pub fn poke(&mut self, address: u16, value: u8) {
        self.bytes[address as usize] = value;
    }

    #[inline]
    pub fn read(&mut self, address: u16) -> u8 {
        if self.tracking {
            self.access[address as usize] |= access::READ;
        }
        self.bytes[address as usize]
    }

    #[inline]
    pub fn write(&mut self, address: u16, value: u8) {
        if self.tracking {
            self.access[address as usize] |= access::WRITE;
        }
        self.bytes[address as usize] = value;
    }

    pub fn load(&mut self, address: u16, bytes: &[u8]) {
        for (offset, byte) in bytes.iter().enumerate() {
            self.poke(address.wrapping_add(offset as u16), *byte);
        }
    }

    /// A copy of one address range, wrapping at the top of memory.
    pub fn slice(&self, address: u16, length: usize) -> Vec<u8> {
        (0..length)
            .map(|offset| self.peek(address.wrapping_add(offset as u16)))
            .collect()
    }

    pub fn access_flags(&self, address: u16) -> u8 {
        self.access[address as usize]
    }

    pub fn access_range(&self, address: u16, length: usize) -> Vec<u8> {
        (0..length)
            .map(|offset| self.access_flags(address.wrapping_add(offset as u16)))
            .collect()
    }

    /// Forget every recorded access, so the editors show only new activity.
    pub fn clear_access(&mut self) {
        self.access.fill(0);
    }

    pub fn as_slice(&self) -> &[u8] {
        self.bytes.as_slice()
    }
}
