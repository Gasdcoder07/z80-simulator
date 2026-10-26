import { useState } from "react";

import { parseHex } from "../format";
import { HexField } from "./HexField";

interface Props {
  onConfirm: (from: number, to: number) => void;
  onCancel: () => void;
}

/** The address range "Save Memory" asks for before writing the HEX file. */
export function MemoryRangeDialog({ onConfirm, onCancel }: Props) {
  const [from, setFrom] = useState(0x0000);
  const [to, setTo] = useState(0xffff);

  const valid = from <= to;

  return (
    <div className="modal-backdrop" onClick={onCancel}>
      <div
        className="modal"
        role="dialog"
        aria-label="Save memory"
        onClick={(event) => event.stopPropagation()}
      >
        <h2>Save Memory</h2>
        <p className="hint">The range is written as Intel HEX, exactly as it sits in memory.</p>

        <div className="modal-row">
          <label>
            From
            <HexField value={from} digits={4} onCommit={setFrom} />
          </label>
          <label>
            To
            <HexField value={to} digits={4} onCommit={setTo} />
          </label>
        </div>

        {!valid && <p className="hint error">The start address is above the end address.</p>}

        <div className="modal-actions">
          <button className="primary" disabled={!valid} onClick={() => onConfirm(from, to)}>
            Save…
          </button>
          <button onClick={onCancel}>Cancel</button>
        </div>
      </div>
    </div>
  );
}

/** Parse a range typed as "0100-01FF". Exported for the menu's quick path. */
export const parseRange = (text: string): [number, number] | null => {
  const [left, right] = text.split("-");
  const from = parseHex(left ?? "");
  const to = parseHex(right ?? "");
  if (from === null || to === null || from > to) return null;
  return [from, to];
};
