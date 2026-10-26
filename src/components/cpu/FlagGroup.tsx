import type { RegisterId } from "../../types";
import { HexField } from "../HexField";

/**
 * The flag bits as Z80 Simulator IDE names them, most significant first.
 * `YF` and `XF` are the undocumented copies of bits 5 and 3 of the result,
 * which the original draws in red.
 */
export const FLAGS: readonly { bit: number; name: string; undocumented: boolean }[] = [
  { bit: 7, name: "SF", undocumented: false },
  { bit: 6, name: "ZF", undocumented: false },
  { bit: 5, name: "YF", undocumented: true },
  { bit: 4, name: "HF", undocumented: false },
  { bit: 3, name: "XF", undocumented: true },
  { bit: 2, name: "PF", undocumented: false },
  { bit: 1, name: "NF", undocumented: false },
  { bit: 0, name: "CF", undocumented: false },
];

interface Props {
  title: string;
  register: RegisterId;
  value: number;
  onCommit: (register: RegisterId, value: number) => void;
}

/** One column of eight flag bits, editable a bit at a time. */
export function FlagGroup({ title, register, value, onCommit }: Props) {
  const setBit = (bit: number, set: boolean) => {
    const mask = 1 << bit;
    onCommit(register, set ? value | mask : value & ~mask);
  };

  return (
    <div className="cpu-group">
      <div className="cpu-group-title">{title}</div>
      <div className="cpu-rows">
        {FLAGS.map(({ bit, name, undocumented }) => (
          <div className="cpu-flag-row" key={name}>
            <span className="cpu-flag-bit">{bit}</span>
            <span className={`cpu-flag-name${undocumented ? " undocumented" : ""}`}>{name}</span>
            <HexField
              value={(value >> bit) & 1}
              digits={1}
              title={`Bit ${bit} of ${title}`}
              onCommit={(entered) => setBit(bit, (entered & 1) !== 0)}
            />
          </div>
        ))}
      </div>
    </div>
  );
}
