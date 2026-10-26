import { useState } from "react";

import { parseHex } from "../../format";
import { useSim } from "../../store/sim";
import { HexField } from "../HexField";

const IM_EXPLANATION = [
  "Mode 0: the bus byte is executed as an instruction, normally an RST.",
  "Mode 1: the bus byte is ignored; the CPU restarts at 0038h.",
  "Mode 2: the bus byte indexes the table at I×256.",
];

/**
 * The interrupt control group: the two flip-flops and the mode, plus the
 * NMI, INT and RESET buttons of the original.
 */
export function InterruptControl() {
  const snapshot = useSim((state) => state.snapshot);
  const send = useSim((state) => state.send);
  const setInterruptState = useSim((state) => state.setInterruptState);
  const [busByte, setBusByte] = useState("FF");

  const iff1 = snapshot?.iff1 ?? false;
  const iff2 = snapshot?.iff2 ?? false;
  const mode = snapshot?.interruptMode ?? 0;
  const parsed = (parseHex(busByte) ?? 0xff) & 0xff;

  return (
    <div className="cpu-group">
      <div className="cpu-group-title">Interrupt control</div>

      <div className="cpu-inline">
        <label className="cpu-inline-field">
          IFF1
          <HexField
            value={iff1 ? 1 : 0}
            digits={1}
            title="Interrupt enable flip-flop 1"
            onCommit={(value) => setInterruptState({ iff1: (value & 1) !== 0 })}
          />
        </label>
        <label className="cpu-inline-field">
          IFF2
          <HexField
            value={iff2 ? 1 : 0}
            digits={1}
            title="Interrupt enable flip-flop 2, the copy NMI saves"
            onCommit={(value) => setInterruptState({ iff2: (value & 1) !== 0 })}
          />
        </label>
        <label className="cpu-inline-field">
          IM
          <HexField
            value={mode}
            digits={1}
            title={IM_EXPLANATION.join("\n")}
            onCommit={(value) => setInterruptState({ mode: Math.min(value & 0xff, 2) })}
          />
        </label>
      </div>

      <div className="cpu-int-buttons">
        <button onClick={() => send({ type: "nmi" })} title="Jump to 0066h">
          NMI
        </button>
        <button
          onClick={() => send({ type: "interrupt", busByte: parsed })}
          disabled={!iff1}
          title={iff1 ? "Raise a maskable interrupt" : "Interrupts are disabled (DI)"}
        >
          INT
        </button>
        <input
          className="hex-field"
          style={{ width: "3.5ch" }}
          value={busByte}
          spellCheck={false}
          title={`The byte a device would place on the bus.\n\n${IM_EXPLANATION[mode] ?? ""}`}
          onChange={(event) => setBusByte(event.target.value)}
        />
        <button onClick={() => send({ type: "reset" })} title="Return the machine to power-on">
          RESET
        </button>
      </div>
    </div>
  );
}
