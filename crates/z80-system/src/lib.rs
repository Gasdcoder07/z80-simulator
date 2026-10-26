//! The simulated Z80 machine: memory, I/O, devices, and the debugging state
//! that the IDE drives them with.

pub mod io;
pub mod machine;
pub mod memory;
pub mod rate;
pub mod trace;

pub use io::{Device, PortEvent, PortSpace};
pub use machine::{LastExecuted, Machine, Options, Statistics, StepOutcome, StopReason};
pub use memory::Memory;
pub use rate::Rate;
pub use trace::{Trace, TraceEntry};
