//! The command surface the window calls into.
//!
//! Anything that advances the CPU is forwarded to the simulation thread;
//! assembly and file formats are pure functions and run inline.

use serde::Serialize;
use tauri::State;
use z80_asm::assemble::ListingLine;
use z80_asm::listing::{self, ListingOptions};
use z80_asm::{assemble, FileLoader};

use z80_system::machine::Options;
use z80_system::Rate;

use crate::sim::{Command, SimHandle, Snapshot};

/// One assembled line, flattened for the listing view.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssembledLine {
    pub address: u16,
    pub bytes: Vec<u8>,
    pub file: String,
    pub line: usize,
    pub text: String,
    pub addressless: bool,
}

impl From<&ListingLine> for AssembledLine {
    fn from(line: &ListingLine) -> Self {
        Self {
            address: line.address,
            bytes: line.bytes.clone(),
            file: line.file.clone(),
            line: line.number,
            text: line.text.clone(),
            addressless: line.addressless,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssembledSymbol {
    pub name: String,
    pub value: i64,
    pub line: usize,
    pub is_address: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssemblyError {
    pub file: String,
    pub line: usize,
    pub message: String,
}

/// Everything the window needs after an assemble.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssemblyResult {
    pub ok: bool,
    pub origin: u16,
    pub listing: Vec<AssembledLine>,
    pub listing_text: String,
    pub symbols: Vec<AssembledSymbol>,
    pub errors: Vec<AssemblyError>,
    pub byte_count: usize,
    pub hex: String,
}

/// Assemble `source` and, when it succeeds, load the result into memory.
#[tauri::command]
pub fn assemble_source(
    sim: State<'_, SimHandle>,
    path: String,
    source: String,
    load: bool,
) -> Result<AssemblyResult, String> {
    let result = assemble(&path, &source, &FileLoader);
    let segments = result.segments();
    let byte_count = segments.iter().map(|(_, bytes)| bytes.len()).sum();

    if load && result.succeeded() {
        sim.send(Command::LoadSegments {
            segments: segments.clone(),
            origin: Some(result.origin),
        })?;
    }

    Ok(AssemblyResult {
        ok: result.succeeded(),
        origin: result.origin,
        listing: result.listing.iter().map(AssembledLine::from).collect(),
        listing_text: listing::render(
            &result,
            ListingOptions { without_opcodes: false, with_symbols: true },
        ),
        symbols: result
            .symbols
            .iter()
            .map(|symbol| AssembledSymbol {
                name: symbol.name.clone(),
                value: symbol.value,
                line: symbol.line,
                is_address: symbol.is_address,
            })
            .collect(),
        errors: result
            .errors
            .iter()
            .map(|error| AssemblyError {
                file: error.file.clone(),
                line: error.line,
                message: error.message.clone(),
            })
            .collect(),
        byte_count,
        hex: z80_asm::formats::to_intel_hex(&segments),
    })
}

/// Send one command to the simulation thread.
#[tauri::command]
pub fn sim_command(sim: State<'_, SimHandle>, command: Command) -> Result<(), String> {
    sim.send(command)
}

/// The latest snapshot, for a window that has just opened.
#[tauri::command]
pub fn sim_snapshot(sim: State<'_, SimHandle>) -> Option<Snapshot> {
    sim.latest()
}

/// Load an Intel HEX or raw binary image into memory.
///
/// A file is treated as Intel HEX when it looks like one; anything else is
/// loaded as a raw image at the configured offset.
#[tauri::command]
pub fn load_image(
    sim: State<'_, SimHandle>,
    bytes: Vec<u8>,
    offset: u16,
) -> Result<String, String> {
    let looks_like_hex = bytes
        .iter()
        .take_while(|byte| byte.is_ascii_whitespace())
        .count()
        < bytes.len()
        && bytes
            .iter()
            .find(|byte| !byte.is_ascii_whitespace())
            .is_some_and(|byte| *byte == b':');

    if looks_like_hex {
        let text = String::from_utf8(bytes).map_err(|_| "the HEX file is not text".to_string())?;
        let parsed = z80_asm::formats::from_intel_hex(&text).map_err(|e| e.to_string())?;
        let low = parsed.iter().map(|(address, _)| *address).min().unwrap_or(0);
        let high = parsed.iter().map(|(address, _)| *address).max().unwrap_or(0);

        // Group the loose bytes into one write per contiguous run.
        let mut segments: Vec<(u16, Vec<u8>)> = Vec::new();
        for (address, byte) in parsed {
            match segments.last_mut() {
                Some((start, bytes)) if start.wrapping_add(bytes.len() as u16) == address => {
                    bytes.push(byte);
                }
                _ => segments.push((address, vec![byte])),
            }
        }

        sim.send(Command::LoadSegments { segments, origin: None })?;
        Ok(format!("Loaded Intel HEX into {low:04X}h-{high:04X}h"))
    } else {
        let length = bytes.len();
        let high = offset.wrapping_add(length.saturating_sub(1) as u16);
        sim.send(Command::LoadSegments {
            segments: vec![(offset, bytes)],
            origin: None,
        })?;
        Ok(format!("Loaded {length} bytes into {offset:04X}h-{high:04X}h"))
    }
}

/// Point the menu's check marks at the simulation's current state. The window
/// calls this when the options or the rate change, whichever control changed
/// them.
#[tauri::command]
pub fn sync_menu(app: tauri::AppHandle, options: Options, rate: Rate) {
    crate::menu::sync(&app, &options, rate);
}

/// Read a range of memory back out as Intel HEX: the File menu's
/// "Save Memory". The simulation thread owns the memory, so the answer comes
/// back over a one-shot channel.
#[tauri::command]
pub fn save_memory(sim: State<'_, SimHandle>, from: u16, to: u16) -> Result<String, String> {
    if from > to {
        return Err("the start address is above the end address".into());
    }
    let (reply, answer) = std::sync::mpsc::channel();
    sim.send(Command::DumpIntelHex { from, to, reply })?;
    answer
        .recv_timeout(std::time::Duration::from_secs(5))
        .map_err(|_| "the simulation thread did not answer".to_string())
}
