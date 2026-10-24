import { useEffect, useRef, useState } from "react";

import { parseHex } from "../format";
import { useSim } from "../store/sim";

/**
 * The dialog behind the "prompt for a value before IN" option: the machine
 * stops just before each port read and waits here for the byte to use.
 * Supplying it resumes the simulation at whatever rate was running.
 */
export function InputPrompt() {
  const snapshot = useSim((state) => state.snapshot);
  const send = useSim((state) => state.send);
  const setOptions = useSim((state) => state.setOptions);
  const [draft, setDraft] = useState("00");
  const field = useRef<HTMLInputElement>(null);

  const waiting = snapshot?.stopReason?.kind === "awaitingInput";
  const instruction = snapshot?.disassembly.find((line) => line.isCurrent)?.text;

  useEffect(() => {
    if (!waiting) return;
    setDraft("00");
    field.current?.focus();
    field.current?.select();
  }, [waiting]);

  if (!waiting) return null;

  const parsed = parseHex(draft);

  const supply = () => {
    if (parsed === null) return;
    send({ type: "queueInput", value: parsed & 0xff });
  };

  // Turning the option off here is the escape hatch: the machine is stopped
  // and nothing else will move it on.
  const stopAsking = async () => {
    if (snapshot) await setOptions({ ...snapshot.options, promptBeforeIn: false });
    await send({ type: "start" });
  };

  return (
    <div className="modal-backdrop">
      <div className="modal">
        <h2>Value for the next port read</h2>
        <p className="hint">
          {instruction ? (
            <>
              The CPU is about to execute <code>{instruction}</code>.
            </>
          ) : (
            "The next instruction reads a port."
          )}
        </p>

        <label className="modal-field">
          Value
          <input
            ref={field}
            className="hex-field"
            style={{ width: "5ch" }}
            value={draft}
            spellCheck={false}
            onChange={(event) => setDraft(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") supply();
            }}
          />
          <span className="dim">hex</span>
        </label>

        <div className="modal-actions">
          <button onClick={stopAsking} title="Clear the option and keep running">
            Stop asking
          </button>
          <button className="primary" onClick={supply} disabled={parsed === null}>
            Supply
          </button>
        </div>
      </div>
    </div>
  );
}
