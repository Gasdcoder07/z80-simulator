/** Shapes mirrored from the Rust side; see src-tauri/src/sim.rs. */

export type Rate =
  | "stepByStep"
  | "slow"
  | "normal"
  | "fast"
  | "extremelyFast"
  | "ultimate";

export interface Registers {
  a: number; f: number;
  b: number; c: number;
  d: number; e: number;
  h: number; l: number;
  a_alt: number; f_alt: number;
  b_alt: number; c_alt: number;
  d_alt: number; e_alt: number;
  h_alt: number; l_alt: number;
  ix: number; iy: number;
  sp: number; pc: number;
  i: number; r: number;
  wz: number;
}

export interface Statistics {
  instructions: number;
  tstates: number;
}

export interface Options {
  haltStopsSimulation: boolean;
  ffPowerOnDefaults: boolean;
  enableUnofficial: boolean;
  promptBeforeIn: boolean;
  clockHz: number;
  startAddress: number;
  objLoadOffset: number;
}

export type StopReason =
  | { kind: "breakpoint"; value: number }
  | { kind: "halt" }
  | { kind: "awaitingInput" };

export interface DisassemblyLine {
  address: number;
  bytes: number[];
  text: string;
  isCurrent: boolean;
  hasBreakpoint: boolean;
}

export interface MemoryWindow {
  address: number;
  bytes: number[];
  /** Bit 0 read, bit 1 written, since the flags were last cleared. */
  access: number[];
}

export interface TraceLine {
  address: number;
  text: string;
  flags: string;
  tstates: number;
}

export interface Snapshot {
  running: boolean;
  rate: Rate;
  registers: Registers;
  flags: string;
  halted: boolean;
  iff1: boolean;
  iff2: boolean;
  interruptMode: number;
  stats: Statistics;
  simulatedSeconds: number;
  stopReason: StopReason | null;
  disassembly: DisassemblyLine[];
  memoryWindows: MemoryWindow[];
  ports: number[];
  portAccess: number[];
  breakpoints: number[];
  trace: TraceLine[];
  options: Options;
}

export interface ViewConfig {
  memoryWindows: [number, number][];
  disassemblyLines: number;
  traceLines: number;
}

export interface AssembledLine {
  address: number;
  bytes: number[];
  file: string;
  line: number;
  text: string;
  addressless: boolean;
}

export interface AssembledSymbol {
  name: string;
  value: number;
  line: number;
  isAddress: boolean;
}

export interface AssemblyError {
  file: string;
  line: number;
  message: string;
}

export interface AssemblyResult {
  ok: boolean;
  origin: number;
  listing: AssembledLine[];
  listingText: string;
  symbols: AssembledSymbol[];
  errors: AssemblyError[];
  byteCount: number;
  hex: string;
}

/** Commands accepted by the simulation thread. */
export type SimCommand =
  | { type: "start" }
  | { type: "stop" }
  | { type: "step" }
  | { type: "reset" }
  | { type: "setRate"; rate: Rate }
  | { type: "setOptions"; options: Options }
  | { type: "setRegisters"; registers: Registers }
  | { type: "setBreakpoints"; addresses: number[] }
  | { type: "toggleBreakpoint"; address: number }
  | { type: "clearMemory" }
  | { type: "writeMemory"; address: number; bytes: number[] }
  | { type: "loadSegments"; segments: [number, number[]][]; origin: number | null }
  | { type: "setPort"; port: number; value: number }
  | { type: "queueInput"; value: number }
  | { type: "nmi" }
  | { type: "interrupt"; busByte: number }
  | { type: "setTracing"; enabled: boolean }
  | { type: "clearTrace" }
  | { type: "setView"; view: ViewConfig }
  | { type: "setUltimateRefreshMillis"; millis: number };
