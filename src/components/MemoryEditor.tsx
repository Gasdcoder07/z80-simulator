import { useState } from "react";

import { hex2, hex4, parseHex, printable } from "../format";
import { useSim } from "../store/sim";

const COLUMNS = 16;
/** Access flag bits, mirrored from z80-system's memory module. */
const READ = 0b01;
const WRITE = 0b10;

interface Props {
  index: number;
}

/**
 * One of the three memory editors. Cells are editable in place and are
 * tinted by whether the program has read or written them.
 */
export function MemoryEditor({ index }: Props) {
  const snapshot = useSim((state) => state.snapshot);
  const pane = useSim((state) => state.memoryPanes[index]);
  const setMemoryPane = useSim((state) => state.setMemoryPane);
  const send = useSim((state) => state.send);
  const [addressDraft, setAddressDraft] = useState(hex4(pane.address));
  const [editing, setEditing] = useState<number | null>(null);
  const [cellDraft, setCellDraft] = useState("");

  const window = snapshot?.memoryWindows[index];
  const rows = Math.ceil((window?.bytes.length ?? 0) / COLUMNS);

  const goTo = (text: string) => {
    const parsed = parseHex(text);
    if (parsed !== null) setMemoryPane(index, { address: parsed & 0xffff });
  };

  const commitCell = (address: number) => {
    const parsed = parseHex(cellDraft);
    if (parsed !== null) {
      send({ type: "writeMemory", address, bytes: [parsed & 0xff] });
    }
    setEditing(null);
  };

  return (
    <div className="memory-editor">
      <div className="memory-toolbar">
        <label>
          Address
          <input
            className="hex-field"
            style={{ width: "6ch" }}
            value={addressDraft}
            spellCheck={false}
            onChange={(event) => setAddressDraft(event.target.value)}
            onBlur={() => goTo(addressDraft)}
            onKeyDown={(event) => {
              if (event.key === "Enter") event.currentTarget.blur();
            }}
          />
        </label>
        <button onClick={() => setMemoryPane(index, { address: (pane.address - 0x100) & 0xffff })}>
          &minus;100h
        </button>
        <button onClick={() => setMemoryPane(index, { address: (pane.address + 0x100) & 0xffff })}>
          +100h
        </button>
        <button
          title="Follow the stack pointer"
          onClick={() => {
            if (!snapshot) return;
            const address = snapshot.registers.sp & 0xfff0;
            setAddressDraft(hex4(address));
            setMemoryPane(index, { address });
          }}
        >
          Focus SP
        </button>
        <button
          title="Follow the program counter"
          onClick={() => {
            if (!snapshot) return;
            const address = snapshot.registers.pc & 0xfff0;
            setAddressDraft(hex4(address));
            setMemoryPane(index, { address });
          }}
        >
          Focus PC
        </button>
      </div>

      {!window ? (
        <div className="panel-empty">Waiting for the simulator…</div>
      ) : (
        <div className="hex-grid">
          <div className="hex-head">
            <span className="hex-address" />
            {Array.from({ length: COLUMNS }, (_, column) => (
              <span key={column} className="hex-col">
                {hex2(column)}
              </span>
            ))}
            <span className="hex-text" />
          </div>

          {Array.from({ length: rows }, (_, row) => {
            const base = (window.address + row * COLUMNS) & 0xffff;
            const slice = window.bytes.slice(row * COLUMNS, (row + 1) * COLUMNS);
            const flags = window.access.slice(row * COLUMNS, (row + 1) * COLUMNS);

            return (
              <div className="hex-row" key={base}>
                <span className="hex-address">{hex4(base)}</span>
                {slice.map((byte, column) => {
                  const address = (base + column) & 0xffff;
                  const flag = flags[column] ?? 0;
                  const classes = [
                    "hex-cell",
                    flag & WRITE ? "written" : flag & READ ? "read" : "",
                    snapshot && address === snapshot.registers.pc ? "at-pc" : "",
                    snapshot && address === snapshot.registers.sp ? "at-sp" : "",
                  ]
                    .filter(Boolean)
                    .join(" ");

                  return editing === address ? (
                    <input
                      key={address}
                      autoFocus
                      className="hex-cell editing"
                      value={cellDraft}
                      spellCheck={false}
                      onChange={(event) => setCellDraft(event.target.value)}
                      onBlur={() => commitCell(address)}
                      onKeyDown={(event) => {
                        if (event.key === "Enter") event.currentTarget.blur();
                        if (event.key === "Escape") setEditing(null);
                      }}
                    />
                  ) : (
                    <span
                      key={address}
                      className={classes}
                      title={`${hex4(address)}h`}
                      onClick={() => {
                        setEditing(address);
                        setCellDraft(hex2(byte));
                      }}
                    >
                      {hex2(byte)}
                    </span>
                  );
                })}
                <span className="hex-text">{slice.map(printable).join("")}</span>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
