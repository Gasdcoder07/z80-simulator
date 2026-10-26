//! A Zilog Z80 CPU core.
//!
//! The core owns nothing but its registers: all memory and port traffic goes
//! through the [`Bus`] trait, so the same CPU drives the simulator, the
//! headless conformance-test runners and anything else that needs it.
//!
//! Accuracy targets, in order of importance:
//!
//! 1. Every documented and undocumented opcode, including the `DD`/`FD`
//!    index-register forms and `DD CB`/`FD CB`.
//! 2. Every flag bit, including the undocumented flags 3 and 5 and the
//!    internal WZ pointer that leaks into `BIT n,(HL)`.
//! 3. Instruction-level cycle counts, which drive the simulated clock.
//!
//! ```
//! use z80_core::{Bus, Cpu};
//!
//! struct Ram([u8; 0x10000]);
//! impl Bus for Ram {
//!     fn read(&mut self, addr: u16) -> u8 { self.0[addr as usize] }
//!     fn write(&mut self, addr: u16, value: u8) { self.0[addr as usize] = value; }
//!     fn input(&mut self, _port: u16) -> u8 { 0xFF }
//!     fn output(&mut self, _port: u16, _value: u8) {}
//! }
//!
//! let mut ram = Ram([0; 0x10000]);
//! ram.0[0] = 0x3E; // LD A,42
//! ram.0[1] = 42;
//! let mut cpu = Cpu::new(false);
//! let step = cpu.step(&mut ram);
//! assert_eq!(cpu.regs.a, 42);
//! assert_eq!(step.tstates, 7);
//! ```

pub mod bus;
pub mod cpu;
pub mod flags;
pub mod registers;

pub use bus::Bus;
pub use cpu::{Cpu, Index, InterruptMode, StepResult};
pub use registers::{RegisterId, Registers};
