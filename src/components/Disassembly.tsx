import { hex2, hex4 } from "../format";
import { useSim } from "../store/sim";

/**
 * A live disassembly from the program counter forward, with breakpoints
 * toggled by clicking the gutter.
 */
export function Disassembly() {
  const snapshot = useSim((state) => state.snapshot);
  const toggleBreakpoint = useSim((state) => state.toggleBreakpoint);

  if (!snapshot) return <div className="panel-empty">Waiting for the simulator…</div>;

  return (
    <div className="disassembly">
      {snapshot.disassembly.map((line) => (
        <div
          key={line.address}
          className={`disassembly-line ${line.isCurrent ? "current" : ""}`}
        >
          <button
            className={`breakpoint ${line.hasBreakpoint ? "set" : ""}`}
            title={line.hasBreakpoint ? "Remove breakpoint" : "Set breakpoint"}
            onClick={() => toggleBreakpoint(line.address)}
          />
          <span className="address">{hex4(line.address)}</span>
          <span className="bytes">{line.bytes.map(hex2).join(" ")}</span>
          <span className="mnemonic">{line.text}</span>
        </div>
      ))}
    </div>
  );
}
