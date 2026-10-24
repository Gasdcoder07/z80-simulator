import { hex2, hex4 } from "../format";
import { useSim } from "../store/sim";

/** The assembler listing: address, machine code and the original source. */
export function Listing() {
  const assembly = useSim((state) => state.assembly);

  if (!assembly) return <div className="panel-empty">Assemble to see the listing.</div>;

  return (
    <div className="listing">
      {assembly.listing.map((line, index) => (
        <div className="listing-line" key={`${line.file}-${line.line}-${index}`}>
          <span className="address">
            {line.addressless && line.bytes.length === 0 ? "" : hex4(line.address)}
          </span>
          <span className="bytes">{line.bytes.map(hex2).join(" ")}</span>
          <span className="source">{line.text}</span>
        </div>
      ))}
    </div>
  );
}
