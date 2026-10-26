import { loadProgram, openSource, saveHex, saveSource } from "../fileActions";
import { hex4 } from "../format";
import { useSim } from "../store/sim";
import type { Rate, StopReason } from "../types";

const RATES: [Rate, string, string][] = [
  ["stepByStep", "Step by step", "One instruction per Step command"],
  ["slow", "Slow", "About 1500 ms per instruction"],
  ["normal", "Normal", "About 250 ms per instruction"],
  ["fast", "Fast", "About 50 ms per instruction"],
  ["extremelyFast", "Extremely fast", "As fast as this machine allows"],
  ["ultimate", "Ultimate", "Full speed with a reduced display refresh"],
];

export function Toolbar() {
  const snapshot = useSim((state) => state.snapshot);
  const dirty = useSim((state) => state.dirty);
  const { send, assemble, setRate } = useSim((state) => state);

  const running = snapshot?.running ?? false;
  const rate = snapshot?.rate ?? "normal";
  const manual = rate === "stepByStep";

  return (
    <header className="toolbar">
      <div className="toolbar-group">
        <button onClick={() => void openSource()}>Open…</button>
        <button onClick={() => void saveSource(false)} disabled={!dirty}>
          Save
        </button>
        <button onClick={() => void saveSource(true)}>Save as…</button>
      </div>

      <div className="toolbar-group">
        <button onClick={() => void loadProgram()} title="Load an Intel HEX or raw binary image">
          Load image…
        </button>
        <button onClick={() => void saveHex()} title="Assemble and write an Intel HEX file">
          Save HEX…
        </button>
      </div>

      <div className="toolbar-group">
        <button className="primary" onClick={() => assemble(true)}>
          Assemble &amp; load
        </button>
      </div>

      <div className="toolbar-group">
        <button
          className="primary"
          onClick={() => send({ type: running ? "stop" : "start" })}
        >
          {running ? "Stop" : manual ? "Start" : "Run"}
        </button>
        <button onClick={() => send({ type: "step" })} disabled={running} title="F2">
          Step
        </button>
        <button onClick={() => send({ type: "reset" })}>Reset</button>
      </div>

      <div className="toolbar-group">
        <label>
          Rate
          <select
            value={rate}
            onChange={(event) => setRate(event.target.value as Rate)}
            title={RATES.find(([value]) => value === rate)?.[2]}
          >
            {RATES.map(([value, label, title]) => (
              <option key={value} value={value} title={title}>
                {label}
              </option>
            ))}
          </select>
        </label>
      </div>

      <div className="toolbar-status">
        {snapshot?.stopReason && <StopBadge reason={snapshot.stopReason} />}
        <span className={running ? "running" : ""}>{running ? "Running" : "Stopped"}</span>
      </div>
    </header>
  );
}

function StopBadge({ reason }: { reason: StopReason }) {
  const text =
    reason.kind === "breakpoint"
      ? `Breakpoint at ${hex4(reason.value)}h`
      : reason.kind === "halt"
        ? "Halted"
        : "Waiting for an input value";
  return <span className="stop-badge">{text}</span>;
}
