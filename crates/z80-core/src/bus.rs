//! The only contact point between the CPU and the outside world.

/// Memory and I/O access for a Z80 CPU.
///
/// Implementations decide what memory and port reads mean; the CPU core itself
/// owns no storage beyond its registers.
pub trait Bus {
    /// Read one byte of memory.
    fn read(&mut self, addr: u16) -> u8;

    /// Write one byte of memory.
    fn write(&mut self, addr: u16, value: u8);

    /// Read a byte from an I/O port. The full 16-bit address is supplied
    /// because real hardware decodes all of it (`IN r,(C)` puts B on A8-A15).
    fn input(&mut self, port: u16) -> u8;

    /// Write a byte to an I/O port.
    fn output(&mut self, port: u16, value: u8);

    /// Byte placed on the data bus when an `IM 0` interrupt is acknowledged.
    /// Real systems almost always supply an `RST n` opcode here; `0xFF`
    /// (`RST 38H`) is the common floating-bus default.
    fn interrupt_data(&mut self) -> u8 {
        0xFF
    }

    /// Called after every instruction with the cycles it consumed, so that
    /// peripherals can advance their own timers.
    fn tick(&mut self, _tstates: u32) {}
}
