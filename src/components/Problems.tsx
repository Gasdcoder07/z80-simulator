import { useSim } from "../store/sim";

interface Props {
  onSelect: (line: number) => void;
}

/** Assembly errors, each one a link back to the offending source line. */
export function Problems({ onSelect }: Props) {
  const assembly = useSim((state) => state.assembly);

  if (!assembly) return <div className="panel-empty">Assemble to see any problems.</div>;
  if (assembly.errors.length === 0)
    return <div className="panel-empty ok">No problems. {assembly.byteCount} bytes assembled.</div>;

  return (
    <div className="problems">
      {assembly.errors.map((error, index) => (
        <button
          className="problem"
          key={index}
          onClick={() => onSelect(error.line)}
        >
          <span className="problem-where">
            {error.file}:{error.line}
          </span>
          <span className="problem-message">{error.message}</span>
        </button>
      ))}
    </div>
  );
}
