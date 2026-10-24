import { useEffect, useState } from "react";

import { hex, parseHex } from "../format";

interface Props {
  value: number;
  digits: number;
  onCommit: (value: number) => void;
  disabled?: boolean;
  title?: string;
}

/**
 * A hex field that only reports a change once the user commits it, so that
 * incoming snapshots never fight with half-typed input.
 */
export function HexField({ value, digits, onCommit, disabled, title }: Props) {
  const formatted = hex(value, digits);
  const [draft, setDraft] = useState(formatted);
  const [editing, setEditing] = useState(false);

  useEffect(() => {
    if (!editing) setDraft(formatted);
  }, [formatted, editing]);

  const commit = () => {
    setEditing(false);
    const parsed = parseHex(draft);
    if (parsed === null) {
      setDraft(formatted);
      return;
    }
    const mask = digits <= 2 ? 0xff : 0xffff;
    onCommit(parsed & mask);
  };

  return (
    <input
      className="hex-field"
      title={title}
      style={{ width: `${digits + 1.5}ch` }}
      value={draft}
      disabled={disabled}
      spellCheck={false}
      onFocus={() => setEditing(true)}
      onChange={(event) => setDraft(event.target.value)}
      onBlur={commit}
      onKeyDown={(event) => {
        if (event.key === "Enter") event.currentTarget.blur();
        if (event.key === "Escape") {
          setDraft(formatted);
          setEditing(false);
          event.currentTarget.blur();
        }
      }}
    />
  );
}
