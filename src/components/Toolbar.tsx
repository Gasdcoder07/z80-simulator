import { open, save } from "@tauri-apps/plugin-dialog";
import { readFile, readTextFile, writeTextFile } from "@tauri-apps/plugin-fs";

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

const ASM_FILTERS = [{ name: "Assembler source", extensions: ["asm", "z80", "s", "txt"] }];

export function Toolbar() {
  const snapshot = useSim((state) => state.snapshot);
  const { source, path, dirty } = useSim((state) => state);
  const { send, assemble, setRate, setSource, setPath, markSaved, setStatus, loadImage } =
    useSim((state) => state);

  const running = snapshot?.running ?? false;
  const rate = snapshot?.rate ?? "normal";
  const manual = rate === "stepByStep";

  const openSource = async () => {
    const chosen = await open({ multiple: false, filters: ASM_FILTERS });
    if (typeof chosen !== "string") return;
    setSource(await readTextFile(chosen));
    setPath(chosen);
    markSaved();
    setStatus(`Opened ${chosen}`);
  };

  const saveSource = async (forceDialog: boolean) => {
    let target = path;
    if (forceDialog || !path.includes("/")) {
      const chosen = await save({ defaultPath: path, filters: ASM_FILTERS });
      if (!chosen) return;
      target = chosen;
      setPath(chosen);
    }
    await writeTextFile(target, source);
    markSaved();
    setStatus(`Saved ${target}`);
  };

  const loadProgram = async () => {
    const chosen = await open({
      multiple: false,
      filters: [{ name: "Program image", extensions: ["hex", "obj", "bin", "com", "rom"] }],
    });
    if (typeof chosen !== "string") return;
    const bytes = await readFile(chosen);
    await loadImage(Array.from(bytes), snapshot?.options.objLoadOffset ?? 0);
  };

  const saveHex = async () => {
    const result = await assemble(false);
    if (!result?.ok) {
      setStatus("Fix the assembly errors before saving a HEX file.");
      return;
    }
    const chosen = await save({
      defaultPath: path.replace(/\.[^.]+$/, "") + ".hex",
      filters: [{ name: "Intel HEX", extensions: ["hex"] }],
    });
    if (!chosen) return;
    await writeTextFile(chosen, result.hex);
    setStatus(`Saved ${chosen}`);
  };

  return (
    <header className="toolbar">
      <div className="toolbar-group">
        <button onClick={openSource}>Open…</button>
        <button onClick={() => saveSource(false)} disabled={!dirty}>
          Save
        </button>
        <button onClick={() => saveSource(true)}>Save as…</button>
      </div>

      <div className="toolbar-group">
        <button onClick={loadProgram} title="Load an Intel HEX or raw binary image">
          Load image…
        </button>
        <button onClick={saveHex} title="Assemble and write an Intel HEX file">
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
