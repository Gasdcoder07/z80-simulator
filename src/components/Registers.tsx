import { duration, hex4, word } from "../format";
import { useSim } from "../store/sim";
import type { Registers as Regs } from "../types";
import { HexField } from "./HexField";

const FLAG_NAMES = ["S", "Z", "F5", "H", "F3", "P/V", "N", "C"];

/** The main register display. Editable whenever the CPU is not running. */
export function Registers() {
  const snapshot = useSim((state) => state.snapshot);
  const setRegisters = useSim((state) => state.setRegisters);

  if (!snapshot) return <div className="panel-empty">Waiting for the simulator…</div>;

  const r = snapshot.registers;
  const locked = snapshot.running;

  const update = (change: Partial<Regs>) => setRegisters({ ...r, ...change });
  const pair = (high: keyof Regs, low: keyof Regs) =>
    word(r[high] as number, r[low] as number);
  const setPair = (high: keyof Regs, low: keyof Regs, value: number) =>
    update({ [high]: (value >> 8) & 0xff, [low]: value & 0xff } as Partial<Regs>);

  return (
    <div className="registers">
      <div className="register-grid">
        {([
          ["AF", "a", "f"],
          ["BC", "b", "c"],
          ["DE", "d", "e"],
          ["HL", "h", "l"],
        ] as const).map(([label, high, low]) => (
          <div className="register-row" key={label}>
            <span className="register-label">{label}</span>
            <HexField
              value={pair(high, low)}
              digits={4}
              disabled={locked}
              onCommit={(value) => setPair(high, low, value)}
            />
            <span className="register-label dim">{label}&apos;</span>
            <HexField
              value={word(r[`${high}_alt` as keyof Regs] as number, r[`${low}_alt` as keyof Regs] as number)}
              digits={4}
              disabled={locked}
              onCommit={(value) =>
                update({
                  [`${high}_alt`]: (value >> 8) & 0xff,
                  [`${low}_alt`]: value & 0xff,
                } as Partial<Regs>)
              }
            />
          </div>
        ))}

        {([
          ["IX", "ix"],
          ["IY", "iy"],
          ["SP", "sp"],
          ["PC", "pc"],
        ] as const).map(([label, key]) => (
          <div className="register-row" key={label}>
            <span className="register-label">{label}</span>
            <HexField
              value={r[key]}
              digits={4}
              disabled={locked}
              onCommit={(value) => update({ [key]: value } as Partial<Regs>)}
            />
            <span />
            <span />
          </div>
        ))}

        <div className="register-row">
          <span className="register-label">I</span>
          <HexField
            value={r.i}
            digits={2}
            disabled={locked}
            onCommit={(value) => update({ i: value })}
          />
          <span className="register-label dim">R</span>
          <HexField
            value={r.r}
            digits={2}
            disabled={locked}
            onCommit={(value) => update({ r: value })}
          />
        </div>

        <div className="register-row">
          <span className="register-label dim" title="The undocumented internal pointer">
            WZ
          </span>
          <span className="register-static">{hex4(r.wz)}</span>
          <span />
          <span />
        </div>
      </div>

      <div className="flags">
        {FLAG_NAMES.map((name, index) => {
          const bit = 0x80 >> index;
          const set = (r.f & bit) !== 0;
          return (
            <button
              key={name}
              className={`flag ${set ? "on" : ""}`}
              disabled={locked}
              title={`Toggle the ${name} flag`}
              onClick={() => update({ f: r.f ^ bit })}
            >
              <span className="flag-name">{name}</span>
              <span className="flag-bit">{set ? 1 : 0}</span>
            </button>
          );
        })}
      </div>

      <dl className="stats">
        <div>
          <dt>Instructions</dt>
          <dd>{snapshot.stats.instructions.toLocaleString()}</dd>
        </div>
        <div>
          <dt>Clock cycles</dt>
          <dd>{snapshot.stats.tstates.toLocaleString()}</dd>
        </div>
        <div>
          <dt>Simulated time</dt>
          <dd>{duration(snapshot.simulatedSeconds)}</dd>
        </div>
        <div>
          <dt>Interrupts</dt>
          <dd>
            IM {snapshot.interruptMode} · IFF1 {snapshot.iff1 ? "1" : "0"} · IFF2{" "}
            {snapshot.iff2 ? "1" : "0"}
          </dd>
        </div>
      </dl>
    </div>
  );
}
