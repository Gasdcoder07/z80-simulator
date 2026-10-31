# Z80 Simulator IDE for macOS and Linux

A Z80 assembler, disassembler, debugger and hardware simulator in one native
macOS application: the workflow of Oshonsoft's Windows-only *Z80 Simulator
IDE*, with its source and object file formats, on a Mac or a Linux machine.

The CPU, assembler and simulated machine are Rust; the window is React and
TypeScript over Monaco, wrapped in Tauri 2. The simulation runs on a thread of
its own and pushes state snapshots to the window, so the interface stays
responsive at every speed.

## Layout

| Path | What it is |
| --- | --- |
| `crates/z80-core` | The CPU. No memory or I/O of its own; everything goes through the `Bus` trait. |
| `crates/z80-asm` | Assembler, disassembler, Intel HEX and listing output. |
| `crates/z80-system` | The simulated machine: 64K memory with access tracking, the I/O port space, devices, rate control, breakpoints and the execution trace. |
| `crates/z80-basic` | BASIC compiler. Not written yet. |
| `src-tauri` | The macOS application: the simulation thread and the commands the window calls. |
| `src` | The window itself: React, TypeScript and Monaco. |
| `tests/conformance` | The ZEXDOC and ZEXALL binaries the CPU is tested against. |

## Requirements

- macOS, with the Xcode command line tools, or Linux (see below)
- Rust 1.77 or newer
- Node 18 or newer

### Linux

Tauri needs WebKitGTK and a few system libraries.

Arch / CachyOS:

```
sudo pacman -S --needed webkit2gtk-4.1 base-devel curl wget file openssl \
    appmenu-gtk-module libappindicator-gtk3 librsvg xdotool rust nodejs npm
```

Debian / Ubuntu:

```
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev \
    libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

On Wayland compositors such as Hyprland, if the window opens blank or glitchy, try
`WEBKIT_DISABLE_DMABUF_RENDERER=1 npm run tauri dev`. File dialogs need an XDG desktop
portal (`xdg-desktop-portal-gtk`, plus the one for your compositor).

## Building and running

```sh
npm install
npm run tauri dev          # development, with hot reload
npm run tauri build        # bundles for your platform (.app/.dmg, .deb/.rpm/.AppImage)
                           # in src-tauri/target/release/bundle
```

`npm run build` type-checks the front end and builds it without Tauri;
`cargo build` builds the Rust workspace on its own.

## Using it

**F9** assembles the source and loads it into memory, **F1** starts the
simulation, **F2** steps one instruction and **F3** stops. `Cmd-O` loads a
program, `Cmd-Shift-O` opens a source file and `Cmd-S` saves it.

The **Simulation ▸ Rate** menu runs the machine step by step, slow, normal,
fast, extremely fast, or as fast as the host will go. **Tools** opens the three
memory editors, the disassembler, the I/O port editor, the breakpoints manager
and the execution log. **Options** holds the clock frequency, the starting
address, the OBJ load offset, the power-on fill byte, whether `HALT` stops the
simulation, whether unofficial instructions are allowed, and whether an `IN`
instruction prompts for its value.

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

## File formats

Intel HEX (`.hex`), raw binary (`.obj`, loaded at the configured offset),
assembler source (`.asm`) and the listing the assembler writes beside it.

## State of the work

Done: the CPU, the assembler and disassembler, the file formats, the simulated
machine, and the debugging interface (registers, flags, three memory editors
with read/write tracking, breakpoints, the disassembly view, the I/O port
editor, the execution trace, the interrupt interface and the options).

Still to come: the peripheral devices and terminals, the BASIC compiler, the
ZX Spectrum ROM interface and the external module protocol.

## License

MIT.
