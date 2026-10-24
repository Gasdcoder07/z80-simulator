import { useState } from "react";

import { parseHex } from "../format";
import { useSim } from "../store/sim";

/**
 * The interrupt interface: the NMI, INT and RESET buttons of the original,
 * with the byte a device would put on the bus during the acknowledge cycle.
 */
export function Interrupts() {
  const snapshot = useSim((state) => state.snapshot);
  const send = useSim((state) => state.send);
  const [busByte, setBusByte] = useState("FF");

  const mode = snapshot?.interruptMode ?? 0;
  const parsed = parseHex(busByte) ?? 0xff;

  const explanation = [
    "Mode 0: the byte is executed as an instruction, normally an RST.",
    "Mode 1: the byte is ignored; the CPU restarts at 0038h.",
    "Mode 2: the byte indexes the table at I×256.",
  ][mode];

  return (
    <div className="interrupts">
      <div className="panel-toolbar">
        <button onClick={() => send({ type: "nmi" })} title="Jump to 0066h">
          NMI
        </button>
        <button
          onClick={() => send({ type: "interrupt", busByte: parsed & 0xff })}
          disabled={!snapshot?.iff1}
          title={snapshot?.iff1 ? "Raise a maskable interrupt" : "Interrupts are disabled (DI)"}
        >
          INT
        </button>
        <label>
          Bus byte
          <input
            className="hex-field"
            style={{ width: "4ch" }}
            value={busByte}
            spellCheck={false}
            onChange={(event) => setBusByte(event.target.value)}
          />
        </label>
        <button onClick={() => send({ type: "reset" })}>RESET</button>
      </div>
      <p className="hint">{explanation}</p>
    </div>
  );
}
