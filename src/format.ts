/** Hex and byte formatting shared by every panel. */

export const hex = (value: number, digits: number): string =>
  value.toString(16).toUpperCase().padStart(digits, "0");

export const hex2 = (value: number): string => hex(value & 0xff, 2);
export const hex4 = (value: number): string => hex(value & 0xffff, 4);

/** `0x1F` in the assembler's own notation, as the disassembler writes it. */
export const asmHex = (value: number, digits: number): string => {
  const text = `${hex(value, digits)}h`;
  return /^[A-F]/.test(text) ? `0${text}` : text;
};

export const word = (high: number, low: number): number =>
  ((high << 8) | low) & 0xffff;

/** Printable ASCII for the hex editor's character column. */
export const printable = (byte: number): string =>
  byte >= 0x20 && byte < 0x7f ? String.fromCharCode(byte) : ".";

/**
 * Parse a value typed into a hex field.
 *
 * These fields are hexadecimal, as in any hex editor: `77` means 77h, not
 * seventy-seven. A `$`, `0x` or `#` prefix and an `h` suffix are accepted and
 * redundant. Returns null when the text is not a hex number.
 */
export const parseHex = (text: string): number | null => {
  const body = text
    .trim()
    .replace(/^[$#]/, "")
    .replace(/^0x/i, "")
    .replace(/h$/i, "");
  if (!/^[0-9a-f]+$/i.test(body)) return null;
  const value = parseInt(body, 16);
  return Number.isNaN(value) ? null : value;
};

/** Seconds as a readable duration, from nanoseconds up to minutes. */
export const duration = (seconds: number): string => {
  if (seconds === 0) return "0 s";
  if (seconds < 1e-6) return `${(seconds * 1e9).toFixed(0)} ns`;
  if (seconds < 1e-3) return `${(seconds * 1e6).toFixed(2)} µs`;
  if (seconds < 1) return `${(seconds * 1e3).toFixed(3)} ms`;
  if (seconds < 60) return `${seconds.toFixed(4)} s`;
  const minutes = Math.floor(seconds / 60);
  return `${minutes} min ${(seconds - minutes * 60).toFixed(2)} s`;
};
