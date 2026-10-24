//! The simulated machine: a CPU wired to 64K of memory, an I/O port space
//! and whatever devices are attached, plus the debugging state that the IDE
//! drives it with.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use z80_asm::disasm::{disassemble_one, Instruction};
use z80_asm::formats;
use z80_core::{Bus, Cpu, InterruptMode, Registers};

use crate::io::PortSpace;
use crate::memory::Memory;
use crate::trace::{Trace, TraceEntry};

/// Settings that mirror the simulator's Options menu.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Options {
    /// Stop the simulation at `HALT` instead of idling until an interrupt,
    /// which is what a real CPU does.
    pub halt_stops_simulation: bool,
    /// Power-on register contents: all ones rather than all zeros.
    pub ff_power_on_defaults: bool,
    /// Execute the undocumented instructions rather than skipping them.
    pub enable_unofficial: bool,
    /// Ask the user for a byte before each `IN` instead of using the port.
    pub prompt_before_in: bool,
    /// Clock used to convert cycles into simulated real time.
    pub clock_hz: u32,
    /// Where `RESET` and "Start" begin execution.
    pub start_address: u16,
    /// Address offset applied when loading a raw `.obj` image. Intel HEX
    /// files carry their own addresses and ignore this.
    pub obj_load_offset: u16,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            halt_stops_simulation: true,
            ff_power_on_defaults: false,
            enable_unofficial: true,
            prompt_before_in: false,
            clock_hz: 4_000_000,
            start_address: 0x0000,
            obj_load_offset: 0x0000,
        }
    }
}

/// Counters reported when the simulation stops.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Statistics {
    pub instructions: u64,
    pub tstates: u64,
}

impl Statistics {
    /// How long the program would have taken on real hardware.
    pub fn simulated_seconds(&self, clock_hz: u32) -> f64 {
        if clock_hz == 0 {
            0.0
        } else {
            self.tstates as f64 / clock_hz as f64
        }
    }
}

/// Why the simulation came to a stop.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "value")]
pub enum StopReason {
    /// Execution reached an address with a breakpoint on it.
    Breakpoint(u16),
    /// A `HALT` was executed and the halt-stops option is on.
    Halt,
    /// An `IN` is next and the user has to supply its value first.
    AwaitingInput,
}

/// The result of advancing the machine by one instruction.
#[derive(Clone, Copy, Debug)]
pub struct StepOutcome {
    pub tstates: u32,
    pub stop: Option<StopReason>,
}

pub struct Machine {
    pub cpu: Cpu,
    pub memory: Memory,
    pub ports: PortSpace,
    pub trace: Trace,
    pub breakpoints: BTreeSet<u16>,
    pub options: Options,
    pub stats: Statistics,
    /// An interrupt raised from the user interface, with the byte the
    /// "device" places on the bus. Cleared once accepted.
    manual_interrupt: Option<u8>,
}

impl Default for Machine {
    fn default() -> Self {
        Self::new(Options::default())
    }
}

impl Machine {
    pub fn new(options: Options) -> Self {
        let mut cpu = Cpu::new(options.ff_power_on_defaults);
        cpu.allow_undocumented = options.enable_unofficial;
        cpu.regs.pc = options.start_address;

        Self {
            cpu,
            memory: Memory::new(),
            ports: PortSpace::new(),
            trace: Trace::default(),
            breakpoints: BTreeSet::new(),
            options,
            stats: Statistics::default(),
            manual_interrupt: None,
        }
    }

    /// Return the machine to its power-on state, keeping memory intact.
    pub fn reset(&mut self) {
        self.cpu = Cpu::new(self.options.ff_power_on_defaults);
        self.cpu.allow_undocumented = self.options.enable_unofficial;
        self.cpu.regs.pc = self.options.start_address;
        self.stats = Statistics::default();
        self.manual_interrupt = None;
        self.memory.clear_access();
        self.ports.clear_access();
        self.trace.clear();
    }

    pub fn apply_options(&mut self, options: Options) {
        self.cpu.allow_undocumented = options.enable_unofficial;
        self.options = options;
    }

    // ---- program loading ---------------------------------------------

    /// Load an Intel HEX image. The addresses come from the file itself.
    pub fn load_intel_hex(&mut self, text: &str) -> Result<(u16, u16), String> {
        let bytes = formats::from_intel_hex(text).map_err(|error| error.to_string())?;
        if bytes.is_empty() {
            return Err("the HEX file contains no data".to_string());
        }

        let low = bytes.iter().map(|(address, _)| *address).min().unwrap();
        let high = bytes.iter().map(|(address, _)| *address).max().unwrap();
        for (address, byte) in bytes {
            self.memory.poke(address, byte);
        }
        Ok((low, high))
    }

    /// Load a raw binary image at the configured offset.
    pub fn load_binary(&mut self, bytes: &[u8]) -> (u16, u16) {
        let base = self.options.obj_load_offset;
        self.memory.load(base, bytes);
        (base, base.wrapping_add(bytes.len().saturating_sub(1) as u16))
    }

    /// Save a memory range as Intel HEX.
    pub fn save_intel_hex(&self, from: u16, to: u16) -> String {
        let length = to as usize - from as usize + 1;
        formats::to_intel_hex(&[(from, self.memory.slice(from, length))])
    }

    // ---- inspection ----------------------------------------------------

    /// Decode the instruction at `address` without disturbing access flags.
    pub fn disassemble_at(&self, address: u16) -> Instruction {
        let memory = &self.memory;
        let mut read = |address: u16| memory.peek(address);
        disassemble_one(&mut read, address)
    }

    /// Decode `count` instructions starting at `address`.
    pub fn disassemble_from(&self, address: u16, count: usize) -> Vec<Instruction> {
        let mut out = Vec::with_capacity(count);
        let mut cursor = address;
        for _ in 0..count {
            let instruction = self.disassemble_at(cursor);
            cursor = instruction.next();
            out.push(instruction);
        }
        out
    }

    pub fn registers(&self) -> &Registers {
        &self.cpu.regs
    }

    pub fn registers_mut(&mut self) -> &mut Registers {
        &mut self.cpu.regs
    }

    // ---- interrupts ----------------------------------------------------

    /// Press the NMI button.
    pub fn request_nmi(&mut self) {
        self.cpu.request_nmi();
    }

    /// Press the INT button. `bus_byte` is what the interrupting device
    /// would place on the data bus: an `RST` opcode in mode 0, the low half
    /// of a vector-table address in mode 2, and ignored in mode 1.
    pub fn request_interrupt(&mut self, bus_byte: u8) {
        self.manual_interrupt = Some(bus_byte);
    }

    pub fn interrupt_mode(&self) -> InterruptMode {
        self.cpu.interrupt_mode
    }

    // ---- execution -----------------------------------------------------

    /// True when the next instruction reads a port, so the user has to be
    /// asked for its value first.
    fn next_instruction_reads_a_port(&self) -> bool {
        let text = self.disassemble_at(self.cpu.regs.pc).text;
        text.starts_with("IN ") || text.starts_with("INI") || text.starts_with("IND")
    }

    /// Advance by one instruction.
    pub fn step(&mut self) -> StepOutcome {
        if self.options.prompt_before_in
            && self.ports.input_queue.is_empty()
            && self.next_instruction_reads_a_port()
        {
            return StepOutcome { tstates: 0, stop: Some(StopReason::AwaitingInput) };
        }

        // Decide what the interrupt controller is presenting this cycle.
        let device_vector = self.ports.pending_interrupt();
        let vector = self.manual_interrupt.or(device_vector);
        self.cpu.set_int_line(vector.is_some());

        // Capture the instruction before it runs; afterwards the PC has
        // moved and the bytes may have been overwritten.
        let pending_trace = if self.trace.enabled {
            Some(self.disassemble_at(self.cpu.regs.pc))
        } else {
            None
        };

        let result = {
            let mut bus = SystemBus {
                memory: &mut self.memory,
                ports: &mut self.ports,
                interrupt_byte: vector.unwrap_or(0xFF),
            };
            self.cpu.step(&mut bus)
        };

        if result.interrupt_taken {
            self.manual_interrupt = None;
        }

        self.stats.instructions += 1;
        self.stats.tstates += result.tstates as u64;

        if let Some(instruction) = pending_trace {
            self.trace.push(TraceEntry {
                address: instruction.address,
                bytes: instruction.bytes,
                text: instruction.text,
                registers: self.cpu.regs,
                tstates: self.stats.tstates,
            });
        }

        let stop = if self.cpu.halted && self.options.halt_stops_simulation {
            Some(StopReason::Halt)
        } else if self.breakpoints.contains(&self.cpu.regs.pc) {
            Some(StopReason::Breakpoint(self.cpu.regs.pc))
        } else {
            None
        };

        StepOutcome { tstates: result.tstates, stop }
    }

    /// Run until something stops the machine or `max_instructions` have run.
    /// Returns the reason for stopping, or `None` when the budget ran out,
    /// which lets the caller yield to the user interface and come back.
    pub fn run(&mut self, max_instructions: u64) -> Option<StopReason> {
        for _ in 0..max_instructions {
            if let Some(reason) = self.step().stop {
                return Some(reason);
            }
        }
        None
    }
}

/// The CPU's view of the machine for the duration of one instruction.
struct SystemBus<'a> {
    memory: &'a mut Memory,
    ports: &'a mut PortSpace,
    interrupt_byte: u8,
}

impl Bus for SystemBus<'_> {
    fn read(&mut self, address: u16) -> u8 {
        self.memory.read(address)
    }

    fn write(&mut self, address: u16, value: u8) {
        self.memory.write(address, value);
    }

    fn input(&mut self, port: u16) -> u8 {
        self.ports.input(port)
    }

    fn output(&mut self, port: u16, value: u8) {
        self.ports.output(port, value);
    }

    fn interrupt_data(&mut self) -> u8 {
        self.interrupt_byte
    }

    fn tick(&mut self, tstates: u32) {
        self.ports.tick(tstates);
    }
}
