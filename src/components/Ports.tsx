import { useState } from "react";

import { hex2, parseHex } from "../format";
import { useSim } from "../store/sim";

const COLUMNS = 16;
const READ = 0b01;
const WRITE = 0b10;

/** The I/O ports editor: all 256 ports, editable, tinted by access. */
export function Ports() {
  const snapshot = useSim((state) => state.snapshot);
  const send = useSim((state) => state.send);
  const [editing, setEditing] = useState<number | null>(null);
  const [draft, setDraft] = useState("");

  if (!snapshot) return <div className="panel-empty">Waiting for the simulator…</div>;

  const commit = (port: number) => {
    const parsed = parseHex(draft);
    if (parsed !== null) send({ type: "setPort", port, value: parsed & 0xff });
    setEditing(null);
  };

  return (
    <div className="ports">
      <p className="hint">
        Values written by <code>OUT</code> appear here; a value set here is what
        the next <code>IN</code> reads.
      </p>
      <div className="hex-grid">
        <div className="hex-head">
          <span className="hex-address" />
          {Array.from({ length: COLUMNS }, (_, column) => (
            <span key={column} className="hex-col">
              {hex2(column)}
            </span>
          ))}
        </div>
        {Array.from({ length: 256 / COLUMNS }, (_, row) => (
          <div className="hex-row" key={row}>
            <span className="hex-address">{hex2(row * COLUMNS)}</span>
            {Array.from({ length: COLUMNS }, (_, column) => {
              const port = row * COLUMNS + column;
              const flag = snapshot.portAccess[port] ?? 0;
              const classes = [
                "hex-cell",
                flag & WRITE ? "written" : flag & READ ? "read" : "",
              ]
                .filter(Boolean)
                .join(" ");

              return editing === port ? (
                <input
                  key={port}
                  autoFocus
                  className="hex-cell editing"
                  value={draft}
                  spellCheck={false}
                  onChange={(event) => setDraft(event.target.value)}
                  onBlur={() => commit(port)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter") event.currentTarget.blur();
                    if (event.key === "Escape") setEditing(null);
                  }}
                />
              ) : (
                <span
                  key={port}
                  className={classes}
                  title={`Port ${hex2(port)}h`}
                  onClick={() => {
                    setEditing(port);
                    setDraft(hex2(snapshot.ports[port] ?? 0));
                  }}
                >
                  {hex2(snapshot.ports[port] ?? 0)}
                </span>
              );
            })}
          </div>
        ))}
      </div>
    </div>
  );
}
