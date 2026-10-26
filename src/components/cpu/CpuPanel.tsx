import { useState } from "react";

import { duration, hex4 } from "../../format";
import { useSim } from "../../store/sim";
import type { AssemblyResult, RegisterId } from "../../types";
import { FlagGroup } from "./FlagGroup";
import { InterruptControl } from "./InterruptControl";
import { RegisterByte } from "./RegisterByte";
import { RegisterPopover } from "./RegisterPopover";
import { RegisterWord } from "./RegisterWord";

const MAIN: [RegisterId, string][] = [
  ["a", "A"],
  ["b", "B"],
  ["c", "C"],
  ["d", "D"],
  ["e", "E"],
  ["h", "H"],
  ["l", "L"],
];

const ALTERNATE: [RegisterId, string][] = [
  ["aAlt", "A'"],
  ["bAlt", "B'"],
  ["cAlt", "C'"],
  ["dAlt", "D'"],
  ["eAlt", "E'"],
  ["hAlt", "H'"],
  ["lAlt", "L'"],
];

const WIDE: [RegisterId, string][] = [
  ["ix", "IX"],
  ["sp", "SP"],
  ["iy", "IY"],
  ["pc", "PC"],
];

interface Editing {
  id: RegisterId;
  label: string;
  digits: 2 | 4;
  anchor: DOMRect;
}

/**
 * Where the program counter is, in the source the assembler saw: the exact
 * line when one starts at this address, otherwise the label it followed.
 */
function programLocation(assembly: AssemblyResult | null, path: string, pc: number): string {
  const file = path.split("/").pop() || "untitled.asm";
  const address = hex4(pc);

  if (!assembly) return `${address}h`;

  const line = assembly.listing.find(
    (entry) => !entry.addressless && entry.address === pc && entry.bytes.length > 0,
  );
  if (line) return `${line.file || file}:${line.line}  ${address}h`;

  const label = assembly.symbols
    .filter((symbol) => symbol.isAddress && symbol.value <= pc)
    .sort((left, right) => right.value - left.value)[0];
  if (label) return `${file}  ${label.name}+${pc - label.value}  ${address}h`;

  return `${file}  ${address}h`;
}

/**
 * The CPU view, laid out as Z80 Simulator IDE lays it out: every register on
 * its own, both flag registers bit by bit, and the interrupt controls.
 */
export function CpuPanel() {
  const snapshot = useSim((state) => state.snapshot);
  const setRegister = useSim((state) => state.setRegister);
  const assembly = useSim((state) => state.assembly);
  const path = useSim((state) => state.path);
  const [editing, setEditing] = useState<Editing | null>(null);

  if (!snapshot) return <div className="panel-empty">Waiting for the simulator…</div>;

  const r = snapshot.registers;
  const commit = (register: RegisterId, value: number) => void setRegister(register, value);
  const open = (id: RegisterId, label: string, digits: 2 | 4, anchor: DOMRect) =>
    setEditing({ id, label, digits, anchor });

  const last = snapshot.lastInstruction;
  const lastText = last ? (last.interruptTaken ? "(INT ACK)" : last.text) : "—";
  const nextText = snapshot.disassembly.find((line) => line.isCurrent)?.text ?? "—";

  return (
    <div className="cpu-panel">
      <div className="cpu-location">
        <label htmlFor="cpu-location-field">Program location</label>
        <input
          id="cpu-location-field"
          className="hex-field"
          readOnly
          value={programLocation(assembly, path, r.pc)}
        />
      </div>

      <div className="cpu-body">
        <div className="cpu-group cpu-main">
          <div className="cpu-group-title">Main registers</div>
          <div className="cpu-rows">
            {MAIN.map(([id, label]) => (
              <RegisterByte
                key={id}
                id={id}
                label={label}
                value={r[id] as number}
                onCommit={commit}
                onOpen={open}
              />
            ))}
          </div>
        </div>

        <div className="cpu-group cpu-alt">
          <div className="cpu-group-title">Alternate registers</div>
          <div className="cpu-rows">
            {ALTERNATE.map(([id, label]) => (
              <RegisterByte
                key={id}
                id={id}
                label={label}
                value={r[id] as number}
                onCommit={commit}
                onOpen={open}
              />
            ))}
          </div>
        </div>

        <div className="cpu-right">
          <div className="cpu-group">
            <div className="cpu-group-title">Last instruction</div>
            <div className="cpu-mnemonic" title={last ? `${hex4(last.address)}h` : undefined}>
              {lastText}
            </div>
            <div className="cpu-next">
              <span>Next</span>
              <div className="cpu-mnemonic">{nextText}</div>
            </div>
          </div>

          <div className="cpu-group">
            <div className="cpu-counter">
              <span>Clock cycles counter</span>
              <output title={duration(snapshot.simulatedSeconds)}>
                {snapshot.stats.tstates.toLocaleString()}
              </output>
            </div>
            <div className="cpu-counter">
              <span>Instructions counter</span>
              <output>{snapshot.stats.instructions.toLocaleString()}</output>
            </div>
          </div>

          <div className="cpu-group">
            <div className="cpu-group-title">16-bit registers</div>
            <div className="cpu-wide">
              {WIDE.map(([id, label]) => (
                <RegisterWord
                  key={id}
                  id={id}
                  label={label}
                  value={r[id] as number}
                  onCommit={commit}
                  onOpen={open}
                />
              ))}
            </div>
          </div>

          <div className="cpu-group">
            <div className="cpu-group-title">Special registers</div>
            <div className="cpu-wide">
              <RegisterByte id="i" label="I" value={r.i} onCommit={commit} onOpen={open} />
              <RegisterByte id="r" label="R" value={r.r} onCommit={commit} onOpen={open} />
            </div>
          </div>

          <InterruptControl />
        </div>

        <div className="cpu-mainf">
          <FlagGroup title="Main F register" register="f" value={r.f} onCommit={commit} />
        </div>
        <div className="cpu-altf">
          <FlagGroup
            title="Alternate F register"
            register="fAlt"
            value={r.fAlt}
            onCommit={commit}
          />
        </div>
      </div>

      {editing && (
        <RegisterPopover
          label={editing.label}
          value={r[editing.id] as number}
          digits={editing.digits}
          anchor={editing.anchor}
          onCommit={(value) => commit(editing.id, value)}
          onClose={() => setEditing(null)}
        />
      )}
    </div>
  );
}
