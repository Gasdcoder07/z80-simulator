import type { RegisterId } from "../../types";
import { HexField } from "../HexField";

interface Props {
  id: RegisterId;
  label: string;
  value: number;
  onCommit: (register: RegisterId, value: number) => void;
  onOpen: (id: RegisterId, label: string, digits: 2 | 4, anchor: DOMRect) => void;
}

/** One 8-bit register: the named button of the original plus its hex field. */
export function RegisterByte({ id, label, value, onCommit, onOpen }: Props) {
  return (
    <div className="cpu-reg-row">
      <button
        className="cpu-reg-button"
        title={`${label} in decimal and binary`}
        onClick={(event) => onOpen(id, label, 2, event.currentTarget.getBoundingClientRect())}
      >
        {label}
      </button>
      <HexField value={value} digits={2} onCommit={(entered) => onCommit(id, entered)} />
    </div>
  );
}
