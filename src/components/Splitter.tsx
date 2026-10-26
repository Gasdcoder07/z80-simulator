import { useRef, useState } from "react";

interface Props {
  width: number;
  min: number;
  max: number;
  onChange: (width: number) => void;
  onCommit: (width: number) => void;
}

/**
 * The drag handle between the editor and the debug pane. The pane grows as
 * the pointer moves left, so the delta is negated.
 */
export function Splitter({ width, min, max, onChange, onCommit }: Props) {
  const start = useRef({ x: 0, width });
  const [dragging, setDragging] = useState(false);

  const clamp = (value: number) => Math.min(max, Math.max(min, value));

  return (
    <div
      className={`splitter${dragging ? " dragging" : ""}`}
      role="separator"
      aria-orientation="vertical"
      title="Drag to resize the CPU pane"
      onPointerDown={(event) => {
        event.currentTarget.setPointerCapture(event.pointerId);
        start.current = { x: event.clientX, width };
        setDragging(true);
      }}
      onPointerMove={(event) => {
        if (!dragging) return;
        onChange(clamp(start.current.width - (event.clientX - start.current.x)));
      }}
      onPointerUp={(event) => {
        if (!dragging) return;
        event.currentTarget.releasePointerCapture(event.pointerId);
        setDragging(false);
        onCommit(clamp(start.current.width - (event.clientX - start.current.x)));
      }}
    />
  );
}
