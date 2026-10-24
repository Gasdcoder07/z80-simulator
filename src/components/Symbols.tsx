import { hex4 } from "../format";
import { useSim } from "../store/sim";

/** The symbol map produced by the last assembly. */
export function Symbols() {
  const assembly = useSim((state) => state.assembly);
  const setMemoryPane = useSim((state) => state.setMemoryPane);

  if (!assembly) return <div className="panel-empty">Assemble to see the symbols.</div>;
  if (assembly.symbols.length === 0)
    return <div className="panel-empty">This program defines no symbols.</div>;

  return (
    <div className="symbols">
      <div className="symbol-row head">
        <span>Name</span>
        <span>Value</span>
        <span>Kind</span>
        <span>Line</span>
      </div>
      {assembly.symbols.map((symbol) => (
        <div
          className="symbol-row"
          key={symbol.name}
          title="Show this address in the first memory editor"
          onClick={() =>
            symbol.isAddress && setMemoryPane(0, { address: symbol.value & 0xfff0 })
          }
        >
          <span className="symbol-name">{symbol.name}</span>
          <span>{hex4(symbol.value)}</span>
          <span className="dim">{symbol.isAddress ? "label" : "equ"}</span>
          <span className="dim">{symbol.line}</span>
        </div>
      ))}
    </div>
  );
}
