import { useEffect, useState } from "react";
import type { CSSProperties } from "react";

import { CpuPanel } from "./components/cpu/CpuPanel";
import { Disassembly } from "./components/Disassembly";
import { InputPrompt } from "./components/InputPrompt";
import { Listing } from "./components/Listing";
import { MemoryEditor } from "./components/MemoryEditor";
import { MemoryRangeDialog } from "./components/MemoryRangeDialog";
import { OptionsPanel } from "./components/OptionsPanel";
import { Ports } from "./components/Ports";
import { Problems } from "./components/Problems";
import { SourceEditor } from "./components/SourceEditor";
import { Splitter } from "./components/Splitter";
import { Symbols } from "./components/Symbols";
import { Toolbar } from "./components/Toolbar";
import { Trace } from "./components/Trace";
import { saveMemoryRange } from "./fileActions";
import { listenToMenu, syncMenu } from "./menuBridge";
import { useSim } from "./store/sim";

type BottomTab = "problems" | "listing" | "symbols" | "trace" | "ports" | "options";

/** How wide the CPU pane may be dragged, in pixels. */
const DEBUG_MIN = 528;
const DEBUG_MAX = 900;
const DEBUG_DEFAULT = 560;
const DEBUG_KEY = "z80.debugWidth";

const storedDebugWidth = (): number => {
  const stored = Number.parseInt(localStorage.getItem(DEBUG_KEY) ?? "", 10);
  if (Number.isNaN(stored)) return DEBUG_DEFAULT;
  return Math.min(DEBUG_MAX, Math.max(DEBUG_MIN, stored));
};

const BOTTOM_TABS: [BottomTab, string][] = [
  ["problems", "Problems"],
  ["listing", "Listing"],
  ["symbols", "Symbols"],
  ["trace", "Trace"],
  ["ports", "I/O ports"],
  ["options", "Options"],
];

export default function App() {
  const connect = useSim((state) => state.connect);
  const send = useSim((state) => state.send);
  const assemble = useSim((state) => state.assemble);
  const status = useSim((state) => state.status);
  const snapshot = useSim((state) => state.snapshot);

  const [bottomTab, setBottomTab] = useState<BottomTab>("problems");
  const [memoryTab, setMemoryTab] = useState(0);
  const [revealLine, setRevealLine] = useState<number | null>(null);
  const [debugWidth, setDebugWidth] = useState(storedDebugWidth);
  const [savingMemory, setSavingMemory] = useState(false);

  useEffect(() => {
    let dispose: (() => void) | undefined;
    connect().then((unlisten) => {
      dispose = unlisten;
    });
    return () => dispose?.();
  }, [connect]);

  useEffect(() => {
    let dispose: (() => void) | undefined;
    listenToMenu({
      showMemoryTab: setMemoryTab,
      showBottomTab: (tab) => setBottomTab(tab as BottomTab),
      askMemoryRange: () => setSavingMemory(true),
      focusSource: () => setRevealLine(1),
    }).then((unlisten) => {
      dispose = unlisten;
    });
    return () => dispose?.();
  }, []);

  // The menu's check marks mirror options the Options panel can also change.
  const options = snapshot?.options;
  const rate = snapshot?.rate;
  useEffect(() => {
    if (options && rate) void syncMenu(options, rate);
  }, [options, rate]);

  // The shortcuts the original uses: F2 steps, F5 runs, F6 stops.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "F2") send({ type: "step" });
      else if (event.key === "F5") send({ type: "start" });
      else if (event.key === "F6") send({ type: "stop" });
      else if (event.key === "F9") assemble(true);
      else return;
      event.preventDefault();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [send, assemble]);

  return (
    <div className="app">
      <Toolbar />
      <InputPrompt />
      {savingMemory && (
        <MemoryRangeDialog
          onCancel={() => setSavingMemory(false)}
          onConfirm={(from, to) => {
            setSavingMemory(false);
            void saveMemoryRange(from, to);
          }}
        />
      )}

      <main
        className="workspace"
        style={{ "--debug-width": `${debugWidth}px` } as CSSProperties}
      >
        <section className="pane editor-pane">
          <div className="pane-title">Source</div>
          <div className="editor-host">
            <SourceEditor revealLine={revealLine} />
          </div>
        </section>

        <Splitter
          width={debugWidth}
          min={DEBUG_MIN}
          max={DEBUG_MAX}
          onChange={setDebugWidth}
          onCommit={(width) => {
            setDebugWidth(width);
            localStorage.setItem(DEBUG_KEY, String(width));
          }}
        />

        <section className="pane debug-pane">
          <div className="pane-title">CPU</div>
          <CpuPanel />
          <div className="pane-title">Disassembly</div>
          <Disassembly />
        </section>

        <section className="pane memory-pane">
          <div className="tab-bar">
            {[0, 1, 2].map((index) => (
              <button
                key={index}
                className={`tab ${memoryTab === index ? "active" : ""}`}
                onClick={() => setMemoryTab(index)}
              >
                Memory {index + 1}
              </button>
            ))}
          </div>
          <MemoryEditor index={memoryTab} />
        </section>
      </main>

      <section className="pane bottom-pane">
        <div className="tab-bar">
          {BOTTOM_TABS.map(([id, label]) => (
            <button
              key={id}
              className={`tab ${bottomTab === id ? "active" : ""}`}
              onClick={() => setBottomTab(id)}
            >
              {label}
              {id === "problems" && snapshot && (useSim.getState().assembly?.errors.length ?? 0) > 0 && (
                <span className="badge">{useSim.getState().assembly?.errors.length}</span>
              )}
            </button>
          ))}
        </div>

        <div className="bottom-body">
          {bottomTab === "problems" && <Problems onSelect={setRevealLine} />}
          {bottomTab === "listing" && <Listing />}
          {bottomTab === "symbols" && <Symbols />}
          {bottomTab === "trace" && <Trace />}
          {bottomTab === "ports" && <Ports />}
          {bottomTab === "options" && <OptionsPanel />}
        </div>
      </section>

      <footer className="status-bar">
        <span>{status}</span>
        <span className="dim">F9 assemble · F5 run · F6 stop · F2 step</span>
      </footer>
    </div>
  );
}
