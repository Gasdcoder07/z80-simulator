import { hex4 } from "../format";
import { useSim } from "../store/sim";

/** The simulation log: every executed instruction with its resulting flags. */
export function Trace() {
  const snapshot = useSim((state) => state.snapshot);
  const tracing = useSim((state) => state.tracing);
  const setTracing = useSim((state) => state.setTracing);
  const send = useSim((state) => state.send);

  return (
    <div className="trace">
      <div className="panel-toolbar">
        <label className="checkbox">
          <input
            type="checkbox"
            checked={tracing}
            onChange={(event) => setTracing(event.target.checked)}
          />
          Record a trace
        </label>
        <button onClick={() => send({ type: "clearTrace" })}>Clear</button>
        <span className="hint">
          Recording costs a disassembly per instruction, so leave it off for
          long runs.
        </span>
      </div>

      {!tracing ? (
        <div className="panel-empty">Tracing is off.</div>
      ) : (
        <div className="trace-list">
          <div className="trace-row head">
            <span>Address</span>
            <span>Instruction</span>
            <span>Flags</span>
            <span>Cycles</span>
          </div>
          {(snapshot?.trace ?? []).map((entry, index) => (
            <div className="trace-row" key={`${entry.address}-${index}`}>
              <span>{hex4(entry.address)}</span>
              <span className="mnemonic">{entry.text}</span>
              <span className="flags-text">{entry.flags}</span>
              <span>{entry.tstates.toLocaleString()}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
