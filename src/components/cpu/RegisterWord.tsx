import type { RegisterId } from "../../types";
import { HexField } from "../HexField";

interface Props {
  id: RegisterId;
  label: string;
  value: number;
  onCommit: (register: RegisterId, value: number) => void;
  onOpen: (id: RegisterId, label: string, digits: 2 | 4, anchor: DOMRect) => void;
}

/** One 16-bit register: IX, IY, SP or PC. */
export function RegisterWord({ id, label, value, onCommit, onOpen }: Props) {
  return (
    <div className="cpu-reg-row wide">
      <button
        className="cpu-reg-button"
        title={`${label} in decimal and binary`}
        onClick={(event) => onOpen(id, label, 4, event.currentTarget.getBoundingClientRect())}
      >
        {label}
      </button>
      <HexField value={value} digits={4} onCommit={(entered) => onCommit(id, entered)} />
    </div>
  );
}
