//! The simulation thread.
//!
//! The machine runs on a thread of its own and talks to the window through a
//! command channel in one direction and state snapshots in the other. Running
//! the CPU inside Tauri commands instead would block the webview for as long
//! as each command took, which at the faster rates means permanently.

use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use z80_core::{InterruptMode, RegisterId, Registers};
use z80_system::machine::{Options, StopReason};
use z80_system::{Machine, Rate, Statistics};

/// The event name the window listens on for state updates.
pub const SNAPSHOT_EVENT: &str = "sim://snapshot";

/// Instructions to run between checks for new commands when free-running.
/// Large enough that the channel check costs nothing, small enough that a
/// Stop command still feels immediate.
const FREE_RUN_SLICE: u64 = 100_000;

/// What the window can ask the simulation thread to do.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum Command {
    Start,
    Stop,
    Step,
    Reset,
    SetRate { rate: Rate },
    SetOptions { options: Options },
    SetRegisters { registers: Registers },
    /// Patch a single register. The CPU view edits one field at a time, and
    /// echoing the whole file back would undo whatever the CPU changed since
    /// the snapshot the window is looking at.
    SetRegister { register: RegisterId, value: u16 },
    /// Edit the interrupt state, which lives on the CPU rather than in the
    /// register file. Fields left out keep their current value.
    SetInterruptState {
        #[serde(default)]
        iff1: Option<bool>,
        #[serde(default)]
        iff2: Option<bool>,
        #[serde(default)]
        mode: Option<u8>,
    },
    SetBreakpoints { addresses: Vec<u16> },
    ToggleBreakpoint { address: u16 },
    ClearMemory,
    WriteMemory { address: u16, bytes: Vec<u8> },
    LoadSegments { segments: Vec<(u16, Vec<u8>)>, origin: Option<u16> },
    SetPort { port: u8, value: u8 },
    QueueInput { value: u8 },
    Nmi,
    Interrupt { bus_byte: u8 },
    SetTracing { enabled: bool },
    ClearTrace,
    SetView { view: ViewConfig },
    SetUltimateRefreshMillis { millis: u64 },
    /// Read a range of memory back out as Intel HEX. Built in Rust only —
    /// the reply channel cannot cross the serde boundary — and used by the
    /// File menu's "Save Memory".
    #[serde(skip)]
    DumpIntelHex { from: u16, to: u16, reply: Sender<String> },
}

/// Which slices of machine state the window currently wants to see. Keeping
/// this explicit stops the thread from serialising all 64K sixty times a
/// second.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewConfig {
    /// Up to three memory editor windows, as (address, length) pairs.
    pub memory_windows: Vec<(u16, u16)>,
    /// How many instructions to disassemble ahead of the program counter.
    pub disassembly_lines: usize,
    /// How many trace entries to send.
    pub trace_lines: usize,
}

impl Default for ViewConfig {
    fn default() -> Self {
        Self {
            memory_windows: vec![(0x0000, 256)],
            disassembly_lines: 24,
            trace_lines: 200,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisassemblyLine {
    pub address: u16,
    pub bytes: Vec<u8>,
    pub text: String,
    pub is_current: bool,
    pub has_breakpoint: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryWindow {
    pub address: u16,
    pub bytes: Vec<u8>,
    /// Access flags per byte: bit 0 read, bit 1 written.
    pub access: Vec<u8>,
}

/// The instruction the CPU has just finished, for the debugger's "last
/// instruction" display.
///
/// The mnemonic is decoded here, once per frame, from the address the machine
/// recorded. Self-modifying code therefore shows the bytes as they are now
/// rather than as they were executed, which is the same trade the disassembly
/// pane already makes.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutedInstruction {
    pub address: u16,
    pub bytes: Vec<u8>,
    pub text: String,
    pub tstates: u32,
    pub interrupt_taken: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceLine {
    pub address: u16,
    pub text: String,
    pub flags: String,
    pub tstates: u64,
}

/// Everything the window draws, as of one moment.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub running: bool,
    pub rate: Rate,
    pub registers: Registers,
    /// The flag byte spelled out, upper case for set.
    pub flags: String,
    pub halted: bool,
    pub iff1: bool,
    pub iff2: bool,
    pub interrupt_mode: u8,
    pub stats: Statistics,
    pub simulated_seconds: f64,
    pub stop_reason: Option<StopReason>,
    pub disassembly: Vec<DisassemblyLine>,
    /// The instruction that ran most recently, or `None` before anything has.
    pub last_instruction: Option<ExecutedInstruction>,
    pub memory_windows: Vec<MemoryWindow>,
    pub ports: Vec<u8>,
    pub port_access: Vec<u8>,
    pub breakpoints: Vec<u16>,
    pub trace: Vec<TraceLine>,
    pub options: Options,
}

/// The window's handle on the simulation thread.
pub struct SimHandle {
    sender: Sender<Command>,
    latest: Arc<Mutex<Option<Snapshot>>>,
}

impl SimHandle {
    pub fn send(&self, command: Command) -> Result<(), String> {
        self.sender
            .send(command)
            .map_err(|_| "the simulation thread has stopped".to_string())
    }

    /// The most recent snapshot, for a window that has just opened and has
    /// not received an event yet.
    pub fn latest(&self) -> Option<Snapshot> {
        self.latest.lock().ok()?.clone()
    }
}

/// Start the simulation thread and return a handle to it.
pub fn spawn(app: AppHandle) -> SimHandle {
    let (sender, receiver) = mpsc::channel();
    let latest = Arc::new(Mutex::new(None));
    let shared = Arc::clone(&latest);

    std::thread::Builder::new()
        .name("z80-simulation".to_string())
        .spawn(move || run(app, receiver, shared))
        .expect("the simulation thread could not be started");

    SimHandle { sender, latest }
}

struct State {
    machine: Machine,
    rate: Rate,
    running: bool,
    view: ViewConfig,
    ultimate_interval: Duration,
    stop_reason: Option<StopReason>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            machine: Machine::default(),
            rate: Rate::Normal,
            running: false,
            view: ViewConfig::default(),
            ultimate_interval: Duration::from_millis(500),
            stop_reason: None,
        }
    }
}

fn run(app: AppHandle, receiver: Receiver<Command>, latest: Arc<Mutex<Option<Snapshot>>>) {
    let mut state = State::default();

    let mut last_emit = Instant::now() - Duration::from_secs(1);
    let mut dirty = true;

    loop {
        // While idle there is nothing to do but wait for a command; while
        // running, commands are drained without blocking so the CPU keeps
        // moving.
        if state.running && !state.rate.is_manual() {
            while let Ok(command) = receiver.try_recv() {
                apply(&mut state, command);
                dirty = true;
            }
        } else {
            match receiver.recv_timeout(Duration::from_millis(50)) {
                Ok(command) => {
                    apply(&mut state, command);
                    dirty = true;
                    while let Ok(command) = receiver.try_recv() {
                        apply(&mut state, command);
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }

        if state.running {
            dirty = true;
            match state.rate.delay() {
                // The paced rates run one instruction and then wait, which
                // is what makes single-stepping by eye possible.
                Some(delay) => {
                    advance(&mut state, 1);
                    emit(&app, &latest, &state);
                    last_emit = Instant::now();
                    dirty = false;
                    std::thread::sleep(delay);
                }
                None => advance(&mut state, FREE_RUN_SLICE),
            }
        }

        let interval = state.rate.refresh_interval(state.ultimate_interval);
        if dirty && last_emit.elapsed() >= interval {
            emit(&app, &latest, &state);
            last_emit = Instant::now();
            dirty = false;
        }
    }
}

/// Run up to `budget` instructions, stopping early on a breakpoint or halt.
fn advance(state: &mut State, budget: u64) {
    if let Some(reason) = state.machine.run(budget) {
        state.stop_reason = Some(reason);
        state.running = false;
    }
}

fn apply(state: &mut State, command: Command) {
    match command {
        Command::Start => {
            state.stop_reason = None;
            // Starting from a halt would otherwise stop again immediately.
            if state.machine.cpu.halted {
                state.machine.cpu.halted = false;
            }
            state.running = !state.rate.is_manual();
            if state.rate.is_manual() {
                step_once(state);
            }
        }
        Command::Stop => state.running = false,
        Command::Step => {
            state.running = false;
            step_once(state);
        }
        Command::Reset => {
            state.running = false;
            state.stop_reason = None;
            state.machine.reset();
        }
        Command::SetRate { rate } => {
            state.rate = rate;
            // Tracking every access is pure overhead at the Ultimate rate,
            // where the display is barely refreshed anyway.
            state.machine.memory.tracking = !matches!(rate, Rate::Ultimate);
            if rate.is_manual() {
                state.running = false;
            }
        }
        Command::SetOptions { options } => state.machine.apply_options(options),
        Command::SetRegisters { registers } => *state.machine.registers_mut() = registers,
        Command::SetRegister { register, value } => {
            state.machine.registers_mut().set(register, value);
        }
        Command::SetInterruptState { iff1, iff2, mode } => {
            if let Some(value) = iff1 {
                state.machine.cpu.iff1 = value;
            }
            if let Some(value) = iff2 {
                state.machine.cpu.iff2 = value;
            }
            if let Some(code) = mode {
                state.machine.cpu.interrupt_mode = InterruptMode::from_code(code);
            }
        }
        Command::SetBreakpoints { addresses } => {
            state.machine.breakpoints = addresses.into_iter().collect();
        }
        Command::ToggleBreakpoint { address } => {
            if !state.machine.breakpoints.remove(&address) {
                state.machine.breakpoints.insert(address);
            }
        }
        Command::ClearMemory => state.machine.memory.clear(),
        Command::WriteMemory { address, bytes } => state.machine.memory.load(address, &bytes),
        Command::LoadSegments { segments, origin } => {
            for (address, bytes) in segments {
                state.machine.memory.load(address, &bytes);
            }
            if let Some(origin) = origin {
                let mut options = state.machine.options;
                options.start_address = origin;
                state.machine.apply_options(options);
                state.machine.cpu.regs.pc = origin;
            }
        }
        Command::SetPort { port, value } => state.machine.ports.set_value(port, value),
        // Supplying the awaited byte also resumes the machine. Without
        // this the "prompt before IN" option stops at the first port read
        // with no way forward.
        Command::QueueInput { value } => {
            state.machine.ports.input_queue.push_back(value);
            if state.stop_reason == Some(StopReason::AwaitingInput) {
                state.stop_reason = None;
                if state.rate.is_manual() {
                    step_once(state);
                } else {
                    state.running = true;
                }
            }
        }
        Command::Nmi => state.machine.request_nmi(),
        Command::Interrupt { bus_byte } => state.machine.request_interrupt(bus_byte),
        Command::SetTracing { enabled } => state.machine.trace.enabled = enabled,
        Command::ClearTrace => state.machine.trace.clear(),
        Command::SetView { view } => state.view = view,
        Command::DumpIntelHex { from, to, reply } => {
            let _ = reply.send(state.machine.save_intel_hex(from, to));
        }
        Command::SetUltimateRefreshMillis { millis } => {
            state.ultimate_interval = Duration::from_millis(millis.max(1));
        }
    }
}

fn step_once(state: &mut State) {
    // A single step past a HALT should make progress rather than sit there.
    if state.machine.cpu.halted {
        state.machine.cpu.halted = false;
    }
    state.stop_reason = state.machine.step().stop;
}

fn emit(app: &AppHandle, latest: &Arc<Mutex<Option<Snapshot>>>, state: &State) {
    let snapshot = build(state);
    if let Ok(mut slot) = latest.lock() {
        *slot = Some(snapshot.clone());
    }
    let _ = app.emit(SNAPSHOT_EVENT, snapshot);
}

fn build(state: &State) -> Snapshot {
    let machine = &state.machine;
    let pc = machine.cpu.regs.pc;

    let disassembly = machine
        .disassemble_from(pc, state.view.disassembly_lines)
        .into_iter()
        .map(|instruction| DisassemblyLine {
            is_current: instruction.address == pc,
            has_breakpoint: machine.breakpoints.contains(&instruction.address),
            address: instruction.address,
            bytes: instruction.bytes,
            text: instruction.text,
        })
        .collect();

    let last_instruction = machine.last_executed.map(|last| {
        let instruction = machine.disassemble_at(last.address);
        ExecutedInstruction {
            address: last.address,
            bytes: instruction.bytes,
            text: instruction.text,
            tstates: last.tstates,
            interrupt_taken: last.interrupt_taken,
        }
    });

    let memory_windows = state
        .view
        .memory_windows
        .iter()
        .map(|(address, length)| MemoryWindow {
            address: *address,
            bytes: machine.memory.slice(*address, *length as usize),
            access: machine.memory.access_range(*address, *length as usize),
        })
        .collect();

    let trace = machine
        .trace
        .tail(state.view.trace_lines)
        .into_iter()
        .map(|entry| TraceLine {
            address: entry.address,
            text: entry.text.clone(),
            flags: z80_system::trace::format_flags(entry.registers.f),
            tstates: entry.tstates,
        })
        .collect();

    Snapshot {
        running: state.running,
        rate: state.rate,
        registers: machine.cpu.regs,
        flags: z80_system::trace::format_flags(machine.cpu.regs.f),
        halted: machine.cpu.halted,
        iff1: machine.cpu.iff1,
        iff2: machine.cpu.iff2,
        interrupt_mode: machine.cpu.interrupt_mode.code(),
        stats: machine.stats,
        simulated_seconds: machine.stats.simulated_seconds(machine.options.clock_hz),
        stop_reason: state.stop_reason,
        disassembly,
        last_instruction,
        memory_windows,
        ports: machine.ports.values().to_vec(),
        port_access: machine.ports.access_flags().to_vec(),
        breakpoints: machine.breakpoints.iter().copied().collect(),
        trace,
        options: machine.options,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use z80_system::machine::Options;

    /// Build a machine stopped in front of an `IN A,(00h)` with the
    /// prompt-before-IN option on, which is where the user meets the
    /// input dialog.
    fn awaiting_input(rate: Rate) -> State {
        let mut state = State::default();
        apply(&mut state, Command::SetRate { rate });
        apply(
            &mut state,
            Command::SetOptions {
                options: Options { prompt_before_in: true, ..Options::default() },
            },
        );
        // DB 00h: IN A,(00h).
        apply(&mut state, Command::WriteMemory { address: 0, bytes: vec![0xDB, 0x00] });
        apply(&mut state, Command::Start);
        advance(&mut state, 1);

        assert_eq!(state.stop_reason, Some(StopReason::AwaitingInput));
        assert!(!state.running);
        state
    }

    #[test]
    fn supplying_a_value_resumes_a_running_machine() {
        let mut state = awaiting_input(Rate::Normal);

        apply(&mut state, Command::QueueInput { value: 0x42 });

        assert_eq!(state.stop_reason, None);
        assert!(state.running, "the machine should carry on once the value is in");

        advance(&mut state, 1);
        assert_eq!(state.machine.cpu.regs.a, 0x42);
    }

    #[test]
    fn supplying_a_value_steps_a_manual_machine() {
        let mut state = awaiting_input(Rate::StepByStep);

        apply(&mut state, Command::QueueInput { value: 0x7F });

        // Step by step runs the instruction itself rather than free-running.
        assert!(!state.running);
        assert_eq!(state.stop_reason, None);
        assert_eq!(state.machine.cpu.regs.a, 0x7F);
    }

    #[test]
    fn a_queued_value_alone_does_not_start_a_stopped_machine() {
        let mut state = State::default();
        apply(&mut state, Command::QueueInput { value: 0x01 });

        assert!(!state.running);
        assert_eq!(state.machine.ports.input_queue.len(), 1);
    }

    #[test]
    fn a_single_register_is_patched_without_disturbing_the_rest() {
        let mut state = State::default();
        state.machine.cpu.regs.b = 0x11;

        apply(&mut state, Command::SetRegister { register: RegisterId::A, value: 0x3F });

        assert_eq!(state.machine.cpu.regs.a, 0x3F);
        assert_eq!(state.machine.cpu.regs.b, 0x11, "the patch spilled into another register");
    }

    #[test]
    fn a_byte_register_takes_only_the_low_half_of_a_wide_value() {
        let mut state = State::default();

        apply(&mut state, Command::SetRegister { register: RegisterId::FAlt, value: 0x1234 });

        assert_eq!(state.machine.cpu.regs.f_alt, 0x34);
    }

    #[test]
    fn the_interrupt_state_patches_only_the_fields_it_names() {
        let mut state = State::default();
        state.machine.cpu.iff2 = true;

        apply(
            &mut state,
            Command::SetInterruptState { iff1: Some(true), iff2: None, mode: Some(2) },
        );

        assert!(state.machine.cpu.iff1);
        assert!(state.machine.cpu.iff2, "iff2 was left out and should not have moved");
        assert_eq!(state.machine.cpu.interrupt_mode, InterruptMode::Mode2);
    }

    #[test]
    fn the_snapshot_reports_the_instruction_that_just_ran() {
        let mut state = State::default();
        assert!(build(&state).last_instruction.is_none(), "nothing has run yet");

        // 3E 05: LD A,05h.
        apply(&mut state, Command::WriteMemory { address: 0, bytes: vec![0x3E, 0x05] });
        apply(&mut state, Command::Step);

        let last = build(&state).last_instruction.expect("one instruction has run");
        assert_eq!(last.address, 0x0000);
        assert_eq!(last.text, "LD A,05h");
        assert_eq!(last.tstates, 7);
        assert!(!last.interrupt_taken);
    }
}
