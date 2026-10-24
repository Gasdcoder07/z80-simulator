//! Behaviour of the simulated machine as the debugger drives it.

use z80_asm::{assemble, NoIncludes};
use z80_system::{Machine, Options, StopReason};

/// Build a machine with `source` assembled into it and the PC at its origin.
fn machine_with(source: &str) -> Machine {
    let program = assemble("test.asm", source, &NoIncludes);
    assert!(
        program.succeeded(),
        "assembly failed: {}",
        program.errors.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")
    );

    let mut machine = Machine::new(Options {
        start_address: program.origin,
        ..Options::default()
    });
    for (address, bytes) in program.segments() {
        machine.memory.load(address, &bytes);
    }
    machine.cpu.regs.pc = program.origin;
    machine.cpu.regs.sp = 0xFF00;
    machine
}

#[test]
fn halt_stops_the_simulation_and_counts_cycles() {
    let mut machine = machine_with("    ORG 100h\n    LD A,1\n    INC A\n    HALT\n");

    assert_eq!(machine.run(100), Some(StopReason::Halt));
    assert_eq!(machine.cpu.regs.a, 2);
    assert_eq!(machine.stats.instructions, 3);
    // LD A,n is 7 cycles, INC A is 4, HALT is 4.
    assert_eq!(machine.stats.tstates, 15);

    // At 4 MHz those 15 cycles are 3.75 microseconds.
    let seconds = machine.stats.simulated_seconds(machine.options.clock_hz);
    assert!((seconds - 3.75e-6).abs() < 1e-12, "got {seconds}");
}

#[test]
fn execution_stops_at_a_breakpoint() {
    let mut machine = machine_with(
        "    ORG 100h
    LD A,1
TARGET
    LD B,2
    HALT
",
    );
    let target = 0x0102;
    machine.breakpoints.insert(target);

    assert_eq!(machine.run(100), Some(StopReason::Breakpoint(target)));
    assert_eq!(machine.cpu.regs.pc, target);
    // The breakpoint fires before its instruction runs.
    assert_eq!(machine.cpu.regs.b, 0);
}

#[test]
fn intel_hex_survives_a_save_and_load() {
    let mut machine = machine_with("    ORG 200h\n    DB 1,2,3,4,5\n");
    let text = machine.save_intel_hex(0x0200, 0x0204);

    machine.memory.clear();
    assert_eq!(machine.memory.peek(0x0200), 0);

    let (low, high) = machine.load_intel_hex(&text).unwrap();
    assert_eq!((low, high), (0x0200, 0x0204));
    assert_eq!(machine.memory.slice(0x0200, 5), vec![1, 2, 3, 4, 5]);
}

#[test]
fn a_manual_interrupt_reaches_its_handler_in_mode_2() {
    let mut machine = machine_with(
        "        ORG 100h
        LD   A,80h
        LD   I,A
        IM   2
        EI
        LD   B,1
        LD   B,2
        HALT
        ORG  8020h
        DW   HANDLER
        ORG  9000h
HANDLER LD   C,0AAh
        EI
        RETI
",
    );

    // Run up to the EI, then raise the interrupt. The vector selects entry
    // 20h of the table at I*256, which points at HANDLER.
    machine.run(4);
    machine.request_interrupt(0x20);
    machine.run(100);

    assert_eq!(machine.cpu.regs.c, 0xAA, "the handler did not run");
    assert_eq!(machine.cpu.regs.b, 2, "execution did not resume after the handler");
}

#[test]
fn the_trace_records_instructions_and_flags() {
    let mut machine = machine_with("    ORG 100h\n    LD A,0\n    OR A\n    HALT\n");
    machine.trace.enabled = true;
    machine.run(100);

    let entries: Vec<String> = machine.trace.entries().map(|e| e.text.clone()).collect();
    assert_eq!(entries, vec!["LD A,00h", "OR A", "HALT"]);

    // `OR A` on a zero accumulator sets Z and clears S and C.
    let flags = z80_system::trace::format_flags(machine.trace.tail(2)[0].registers.f);
    assert!(flags.starts_with("sZ"), "unexpected flags {flags}");
    assert!(flags.ends_with('c'), "unexpected flags {flags}");
}

#[test]
fn memory_access_tracking_follows_reads_and_writes() {
    use z80_system::memory::access;

    let mut machine = machine_with("    ORG 100h\n    LD HL,2000h\n    LD (HL),55h\n    HALT\n");
    machine.run(100);

    assert_eq!(machine.memory.peek(0x2000), 0x55);
    assert_eq!(machine.memory.access_flags(0x2000) & access::WRITE, access::WRITE);
    // The opcode bytes themselves were fetched, so they read as read.
    assert_eq!(machine.memory.access_flags(0x0100) & access::READ, access::READ);

    machine.memory.clear_access();
    assert_eq!(machine.memory.access_flags(0x2000), 0);
}
