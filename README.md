# Z80 Simulator IDE for macOS

A Z80 assembler, disassembler, debugger and hardware simulator in one native
macOS application: the workflow of Oshonsoft's Windows-only *Z80 Simulator
IDE*, with its source and object file formats, on a Mac.

## Layout

| Path | What it is |
| --- | --- |
| `crates/z80-core` | The CPU. No memory or I/O of its own; everything goes through the `Bus` trait. |
| `crates/z80-asm` | Assembler, disassembler, Intel HEX and listing output. |
| `crates/z80-system` | The simulated machine: 64K memory with access tracking, the I/O port space, devices, rate control, breakpoints and the execution trace. |
| `crates/z80-basic` | BASIC compiler. Not written yet. |
| `src-tauri` | The macOS application: the simulation thread and the commands the window calls. |
| `src` | The window itself: React, TypeScript and Monaco. |

The simulation runs on a thread of its own and pushes state snapshots to the
window, so the interface stays responsive at every speed.

## Running it

```sh
npm install
npm run tauri dev          # development, with hot reload
npm run tauri build        # a signed .app and .dmg in src-tauri/target/release/bundle
```

Inside the window: **F9** assembles and loads, **F5** runs, **F6** stops and
**F2** steps.

## Accuracy

The CPU implements every documented and undocumented opcode, the undocumented
flags 3 and 5, and the internal WZ (MEMPTR) pointer that `BIT n,(HL)` leaks
into them. Cycle counts are per instruction and drive the simulated clock.

```sh
cargo test                                          # the fast suites
cargo test -p z80-core --release -- --ignored       # ZEXDOC and ZEXALL
```

ZEXDOC and ZEXALL both pass with no errors. They take about two and a half
minutes together and need `--release`; in a debug build they are far too slow.

The assembler is checked three ways: a table of encodings taken from the Zilog
manual, an exhaustive round trip over all seven opcode pages (every opcode is
disassembled, re-assembled and disassembled again), and programs that are
assembled and then actually executed on the CPU.

## Assembler syntax

The syntax follows Z80 Simulator IDE, so course material written for it
assembles here unchanged.

- **Comments** start with `;`.
- **Labels** sit at the left margin, with or without a trailing colon.
- **Constants**: `'A'`, `0FFh`, `$FF`, `0xFF`, `11001100b`, `255`, with an
  optional `#` prefix.
- **Directives**: `ORG`, `EQU` (or `=`), `INCLUDE` (or `#INCLUDE`), `DB`
  (`DEFB`, `BYTE`), `DW` (`DEFW`, `WORD`), `DS` (`DEFS`, `BLOCK`), `END`. A
  leading dot is allowed on any of them.
- **Expressions**: the arithmetic, bitwise and shift operators, parentheses,
  `>` and `<` for the high and low byte, `highw` and `loww` for the high and
  low word, and `$` or `PC` for the current address.
- **Macros** are not supported, matching the original.

## State of the work

Done: the CPU, the assembler and disassembler, the file formats, the simulated
machine, and the debugging interface (registers, flags, three memory editors
with read/write tracking, breakpoints, the disassembly view, the I/O port
editor, the execution trace, the interrupt interface and the options).

Still to come: the peripheral devices and terminals, the BASIC compiler, the
ZX Spectrum ROM interface and the external module protocol. See
`~/.claude/plans/necesito-poder-tener-una-fancy-hollerith.md` for the plan.
