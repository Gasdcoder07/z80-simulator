import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";

import type {
  AssemblyResult,
  Options,
  Rate,
  Registers,
  SimCommand,
  Snapshot,
  ViewConfig,
} from "../types";

const SNAPSHOT_EVENT = "sim://snapshot";

const DEFAULT_SOURCE = `; Sum a table of bytes and leave the total in A.

        ORG   0100h

COUNT   EQU   5

START   LD    HL,TABLE
        LD    B,COUNT
        XOR   A
LOOP    ADD   A,(HL)
        INC   HL
        DJNZ  LOOP
        LD    (TOTAL),A
        HALT

TABLE   DB    1,2,3,4,5
TOTAL   DS    1

        END
`;

export interface MemoryPane {
  address: number;
  length: number;
}

interface SimState {
  snapshot: Snapshot | null;
  assembly: AssemblyResult | null;
  source: string;
  path: string;
  dirty: boolean;
  status: string;
  memoryPanes: MemoryPane[];
  tracing: boolean;

  connect: () => Promise<() => void>;
  send: (command: SimCommand) => Promise<void>;
  setSource: (source: string) => void;
  setPath: (path: string) => void;
  markSaved: () => void;
  setStatus: (status: string) => void;
  assemble: (load: boolean) => Promise<AssemblyResult | null>;
  setRate: (rate: Rate) => Promise<void>;
  setOptions: (options: Options) => Promise<void>;
  setRegisters: (registers: Registers) => Promise<void>;
  toggleBreakpoint: (address: number) => Promise<void>;
  setMemoryPane: (index: number, pane: Partial<MemoryPane>) => Promise<void>;
  setTracing: (enabled: boolean) => Promise<void>;
  loadImage: (bytes: number[], offset: number) => Promise<void>;
}

/** The slices of state the simulation thread should keep sending us. */
const viewOf = (panes: MemoryPane[], tracing: boolean): ViewConfig => ({
  memoryWindows: panes.map((pane) => [pane.address, pane.length]),
  disassemblyLines: 28,
  traceLines: tracing ? 300 : 0,
});

export const useSim = create<SimState>((set, get) => ({
  snapshot: null,
  assembly: null,
  source: DEFAULT_SOURCE,
  path: "program.asm",
  dirty: false,
  status: "Ready",
  memoryPanes: [
    { address: 0x0100, length: 256 },
    { address: 0x0000, length: 256 },
    { address: 0x8000, length: 256 },
  ],
  tracing: false,

  async connect() {
    const unlisten = await listen<Snapshot>(SNAPSHOT_EVENT, (event) => {
      set({ snapshot: event.payload });
    });

    // Catch up on whatever the thread has already produced, then tell it
    // which panes we are showing.
    const current = await invoke<Snapshot | null>("sim_snapshot");
    if (current) set({ snapshot: current });
    const { memoryPanes, tracing } = get();
    await get().send({ type: "setView", view: viewOf(memoryPanes, tracing) });

    return unlisten;
  },

  async send(command) {
    try {
      await invoke("sim_command", { command });
    } catch (error) {
      set({ status: `${error}` });
    }
  },

  setSource(source) {
    set({ source, dirty: true });
  },

  setPath(path) {
    set({ path });
  },

  markSaved() {
    set({ dirty: false });
  },

  setStatus(status) {
    set({ status });
  },

  async assemble(load) {
    const { path, source } = get();
    try {
      const result = await invoke<AssemblyResult>("assemble_source", {
        path,
        source,
        load,
      });
      set({
        assembly: result,
        status: result.ok
          ? `Assembled ${result.byteCount} byte${result.byteCount === 1 ? "" : "s"} at ${result.origin
              .toString(16)
              .toUpperCase()
              .padStart(4, "0")}h`
          : `${result.errors.length} error${result.errors.length === 1 ? "" : "s"}`,
      });
      return result;
    } catch (error) {
      set({ status: `${error}` });
      return null;
    }
  },

  async setRate(rate) {
    await get().send({ type: "setRate", rate });
  },

  async setOptions(options) {
    await get().send({ type: "setOptions", options });
  },

  async setRegisters(registers) {
    await get().send({ type: "setRegisters", registers });
  },

  async toggleBreakpoint(address) {
    await get().send({ type: "toggleBreakpoint", address });
  },

  async setMemoryPane(index, pane) {
    const panes = get().memoryPanes.map((existing, position) =>
      position === index ? { ...existing, ...pane } : existing,
    );
    set({ memoryPanes: panes });
    await get().send({ type: "setView", view: viewOf(panes, get().tracing) });
  },

  async setTracing(enabled) {
    set({ tracing: enabled });
    await get().send({ type: "setTracing", enabled });
    await get().send({
      type: "setView",
      view: viewOf(get().memoryPanes, enabled),
    });
  },

  async loadImage(bytes, offset) {
    try {
      const message = await invoke<string>("load_image", { bytes, offset });
      set({ status: message });
    } catch (error) {
      set({ status: `${error}` });
    }
  },
}));
