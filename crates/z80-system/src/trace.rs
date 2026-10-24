//! The execution trace that backs the Simulation Log Viewer and `LOG.TXT`.

use std::collections::VecDeque;
use std::fmt::Write;

use serde::{Deserialize, Serialize};
use z80_core::Registers;

/// One executed instruction and the register state it left behind.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceEntry {
    pub address: u16,
    pub bytes: Vec<u8>,
    pub text: String,
    pub registers: Registers,
    /// Cumulative clock cycles at the end of this instruction.
    pub tstates: u64,
}

impl TraceEntry {
    /// One line in the layout `LOG.TXT` uses: address, mnemonic, then the
    /// register file and the decoded flags.
    pub fn to_log_line(&self) -> String {
        let r = &self.registers;
        let mut line = format!("{:04X}  {:<20}", self.address, self.text);
        let _ = write!(
            line,
            " AF={:04X} BC={:04X} DE={:04X} HL={:04X} IX={:04X} IY={:04X} SP={:04X}  {}",
            r.af(),
            r.bc(),
            r.de(),
            r.hl(),
            r.ix,
            r.iy,
            r.sp,
            format_flags(r.f)
        );
        line
    }
}

/// The flag byte as the eight letters the debugger shows, lower case for a
/// clear flag and upper case for a set one.
pub fn format_flags(f: u8) -> String {
    const NAMES: [char; 8] = ['S', 'Z', '5', 'H', '3', 'P', 'N', 'C'];
    NAMES
        .iter()
        .enumerate()
        .map(|(index, name)| {
            if f & (0x80 >> index) != 0 {
                *name
            } else {
                name.to_ascii_lowercase()
            }
        })
        .collect()
}

/// A bounded ring of recent instructions.
pub struct Trace {
    entries: VecDeque<TraceEntry>,
    /// How many entries to keep. The log viewer only ever shows the tail,
    /// and an unbounded log would exhaust memory in seconds at full speed.
    pub limit: usize,
    pub enabled: bool,
}

impl Default for Trace {
    fn default() -> Self {
        Self::new(10_000)
    }
}

impl Trace {
    pub fn new(limit: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(limit.min(1024)),
            limit,
            enabled: false,
        }
    }

    pub fn push(&mut self, entry: TraceEntry) {
        if self.entries.len() >= self.limit {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries(&self) -> impl Iterator<Item = &TraceEntry> {
        self.entries.iter()
    }

    /// The most recent `count` entries, oldest first.
    pub fn tail(&self, count: usize) -> Vec<&TraceEntry> {
        let skip = self.entries.len().saturating_sub(count);
        self.entries.iter().skip(skip).collect()
    }

    /// Render the whole trace in `LOG.TXT` form.
    pub fn to_log(&self) -> String {
        let mut out = String::new();
        for entry in &self.entries {
            out.push_str(&entry.to_log_line());
            out.push('\n');
        }
        out
    }
}
