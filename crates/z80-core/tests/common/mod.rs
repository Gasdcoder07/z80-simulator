//! A minimal CP/M 2.2 environment, just enough to run the classic Z80
//! conformance binaries (ZEXDOC, ZEXALL) which are distributed as `.COM`
//! files and report their results through BDOS console calls.

use z80_core::{Bus, Cpu};

pub const LOAD_ADDRESS: u16 = 0x0100;
const BDOS_ENTRY: u16 = 0x0005;
const WARM_BOOT: u16 = 0x0000;

pub struct CpmMachine {
    memory: Box<[u8; 0x10000]>,
    /// Everything the program wrote to the console.
    pub output: String,
}

impl CpmMachine {
    pub fn load(program: &[u8]) -> Self {
        let mut memory = Box::new([0u8; 0x10000]);
        memory[LOAD_ADDRESS as usize..LOAD_ADDRESS as usize + program.len()]
            .copy_from_slice(program);

        // The BDOS entry point just returns; the runner traps the call by
        // watching PC instead of emulating CP/M itself.
        memory[BDOS_ENTRY as usize] = 0xC9;
        // A warm-boot jump at 0x0000 is how a .COM program exits.
        memory[WARM_BOOT as usize] = 0xC3;

        Self { memory, output: String::new() }
    }

    /// Service a BDOS call. Only the two console functions the test
    /// binaries use are implemented.
    fn bdos(&mut self, cpu: &Cpu) {
        match cpu.regs.c {
            // C_WRITE: print the character in E.
            2 => self.output.push(cpu.regs.e as char),
            // C_WRITESTR: print the '$'-terminated string at DE.
            9 => {
                let mut addr = cpu.regs.de();
                loop {
                    let byte = self.memory[addr as usize];
                    if byte == b'$' {
                        break;
                    }
                    self.output.push(byte as char);
                    addr = addr.wrapping_add(1);
                }
            }
            _ => {}
        }
    }
}

impl Bus for CpmMachine {
    fn read(&mut self, addr: u16) -> u8 {
        self.memory[addr as usize]
    }

    fn write(&mut self, addr: u16, value: u8) {
        self.memory[addr as usize] = value;
    }

    fn input(&mut self, _port: u16) -> u8 {
        0xFF
    }

    fn output(&mut self, _port: u16, _value: u8) {}
}

/// Run a CP/M program to completion, returning its console output and the
/// total number of cycles it took.
///
/// `on_line` is called with each completed line of output, so a long-running
/// suite can report progress instead of going silent for minutes.
pub fn run_cpm(program: &[u8], mut on_line: impl FnMut(&str)) -> (String, u64) {
    let mut machine = CpmMachine::load(program);
    let mut cpu = Cpu::new(false);
    cpu.regs.pc = LOAD_ADDRESS;
    cpu.regs.sp = 0xF000;

    let mut tstates: u64 = 0;
    let mut reported = 0usize;

    loop {
        match cpu.regs.pc {
            WARM_BOOT => break,
            BDOS_ENTRY => machine.bdos(&cpu),
            _ => {}
        }

        tstates += cpu.step(&mut machine).tstates as u64;

        // Flush any newly completed lines to the progress callback.
        while let Some(end) = machine.output[reported..].find('\n') {
            let line = machine.output[reported..reported + end].trim_end().to_string();
            reported += end + 1;
            on_line(&line);
        }
    }

    (machine.output, tstates)
}
