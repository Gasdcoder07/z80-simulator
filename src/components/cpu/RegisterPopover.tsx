import { useEffect, useRef, useState } from "react";

import { hex, parseHex } from "../../format";

interface Props {
  label: string;
  value: number;
  digits: 2 | 4;
  anchor: DOMRect;
  onCommit: (value: number) => void;
  onClose: () => void;
}

const parseValue = (text: string, base: 10 | 2): number | null => {
  const trimmed = text.trim();
  if (!trimmed) return null;
  const pattern = base === 10 ? /^\d+$/ : /^[01]+$/;
  if (!pattern.test(trimmed)) return null;
  return Number.parseInt(trimmed, base);
};

/**
 * The editor behind each register's named button: the same value in hex,
 * decimal and binary, with the individual bits clickable.
 */
export function RegisterPopover({ label, value, digits, anchor, onCommit, onClose }: Props) {
  const mask = digits === 2 ? 0xff : 0xffff;
  const width = digits * 4;
  const [draft, setDraft] = useState(value & mask);
  const container = useRef<HTMLDivElement>(null);

  useEffect(() => {
    container.current?.querySelector("input")?.focus();
  }, []);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    const onPointerDown = (event: PointerEvent) => {
      if (!container.current?.contains(event.target as Node)) onClose();
    };
    window.addEventListener("keydown", onKey);
    window.addEventListener("pointerdown", onPointerDown);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("pointerdown", onPointerDown);
    };
  }, [onClose]);

  const commit = () => {
    onCommit(draft & mask);
    onClose();
  };

  const field = (
    text: string,
    base: 10 | 2 | 16,
    onEntered: (parsed: number) => void,
    title: string,
  ) => (
    <input
      className="hex-field"
      value={text}
      spellCheck={false}
      title={title}
      onChange={(event) => {
        const parsed =
          base === 16 ? parseHex(event.target.value) : parseValue(event.target.value, base);
        if (parsed !== null) onEntered(parsed & mask);
      }}
      onKeyDown={(event) => {
        if (event.key === "Enter") commit();
      }}
    />
  );

  return (
    <div
      className="cpu-popover"
      ref={container}
      style={{ top: anchor.bottom + 4, left: anchor.left }}
    >
      <div className="cpu-popover-title">{label}</div>

      <label className="cpu-popover-row">
        <span>Hex</span>
        {field(hex(draft, digits), 16, setDraft, `${label} in hexadecimal`)}
      </label>
      <label className="cpu-popover-row">
        <span>Dec</span>
        {field(String(draft), 10, setDraft, `${label} in decimal`)}
      </label>
      <label className="cpu-popover-row">
        <span>Bin</span>
        {field(draft.toString(2).padStart(width, "0"), 2, setDraft, `${label} in binary`)}
      </label>

      <div className="cpu-popover-bits">
        {Array.from({ length: width }, (_, index) => width - 1 - index).map((bit) => (
          <button
            key={bit}
            className={`cpu-bit${(draft >> bit) & 1 ? " on" : ""}`}
            title={`Toggle bit ${bit}`}
            onClick={() => setDraft((current) => (current ^ (1 << bit)) & mask)}
          >
            {(draft >> bit) & 1}
          </button>
        ))}
      </div>

      <div className="cpu-popover-actions">
        <button className="primary" onClick={commit}>
          Set
        </button>
        <button onClick={onClose}>Cancel</button>
      </div>
    </div>
  );
}
