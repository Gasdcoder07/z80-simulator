import { useSim } from "../store/sim";
import type { Options } from "../types";
import { HexField } from "./HexField";

const TOGGLES: [keyof Options, string, string][] = [
  [
    "haltStopsSimulation",
    "HALT stops the simulation",
    "Off means the CPU idles at a HALT until an interrupt arrives, as real hardware does.",
  ],
  [
    "ffPowerOnDefaults",
    "FF power-on defaults",
    "Start with every register set to FFh instead of 00h.",
  ],
  [
    "enableUnofficial",
    "Enable unofficial instructions",
    "Execute the undocumented opcodes instead of treating them as NOPs.",
  ],
  [
    "promptBeforeIn",
    "Prompt for a value before IN",
    "Pause before each port read so a value can be supplied by hand.",
  ],
];

/** The settings from the simulator's Options menu that affect execution. */
export function OptionsPanel() {
  const snapshot = useSim((state) => state.snapshot);
  const setOptions = useSim((state) => state.setOptions);

  if (!snapshot) return <div className="panel-empty">Waiting for the simulator…</div>;
  const options = snapshot.options;
  const update = (change: Partial<Options>) => setOptions({ ...options, ...change });

  return (
    <div className="options">
      {TOGGLES.map(([key, label, hint]) => (
        <label className="checkbox" key={key} title={hint}>
          <input
            type="checkbox"
            checked={options[key] as boolean}
            onChange={(event) => update({ [key]: event.target.checked } as Partial<Options>)}
          />
          <span>
            {label}
            <small>{hint}</small>
          </span>
        </label>
      ))}

      <label className="field">
        <span>Clock frequency</span>
        <input
          type="number"
          min={1}
          step={1000}
          value={options.clockHz}
          onChange={(event) =>
            update({ clockHz: Math.max(1, Number(event.target.value) || 1) })
          }
        />
        <small>{(options.clockHz / 1e6).toFixed(3)} MHz, used for the simulated time</small>
      </label>

      <label className="field">
        <span>Starting address</span>
        <HexField
          value={options.startAddress}
          digits={4}
          onCommit={(value) => update({ startAddress: value })}
        />
        <small>Where RESET puts the program counter</small>
      </label>

      <label className="field">
        <span>Offset for raw image loading</span>
        <HexField
          value={options.objLoadOffset}
          digits={4}
          onCommit={(value) => update({ objLoadOffset: value })}
        />
        <small>Ignored for Intel HEX files, which carry their own addresses</small>
      </label>
    </div>
  );
}
