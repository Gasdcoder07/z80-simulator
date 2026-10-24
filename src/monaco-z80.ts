/**
 * Monaco setup: the editor is bundled rather than loaded from a CDN, and a
 * small Z80 language definition gives the source view its highlighting and
 * the assembler's own comment and number syntax.
 */
import { loader } from "@monaco-editor/react";
// Only the editor core, not the hundred built-in languages: this one
// ships a Z80 grammar and nothing else, and it saves ~2.5 MB.
import * as monaco from "monaco-editor/esm/vs/editor/editor.api";
import editorWorker from "monaco-editor/esm/vs/editor/editor.worker?worker";

const MNEMONICS = [
  "ADC", "ADD", "AND", "BIT", "CALL", "CCF", "CP", "CPD", "CPDR", "CPI",
  "CPIR", "CPL", "DAA", "DEC", "DI", "DJNZ", "EI", "EX", "EXX", "HALT", "IM",
  "IN", "INC", "IND", "INDR", "INI", "INIR", "JP", "JR", "LD", "LDD", "LDDR",
  "LDI", "LDIR", "NEG", "NOP", "OR", "OTDR", "OTIR", "OUT", "OUTD", "OUTI",
  "POP", "PUSH", "RES", "RET", "RETI", "RETN", "RL", "RLA", "RLC", "RLCA",
  "RLD", "RR", "RRA", "RRC", "RRCA", "RRD", "RST", "SBC", "SCF", "SET", "SLA",
  "SLI", "SLL", "SRA", "SRL", "SUB", "XOR",
];

const DIRECTIVES = [
  "ORG", "END", "EQU", "DB", "DEFB", "BYTE", "DW", "DEFW", "WORD", "DS",
  "DEFS", "BLOCK", "INCLUDE",
];

const REGISTERS = [
  "A", "B", "C", "D", "E", "H", "L", "F", "I", "R", "AF", "BC", "DE", "HL",
  "SP", "IX", "IY", "IXH", "IXL", "IYH", "IYL",
  "NZ", "Z", "NC", "PO", "PE", "P", "M",
];

export const Z80_LANGUAGE = "z80";

let configured = false;

export function setupMonaco(): void {
  if (configured) return;
  configured = true;

  // Bundle the worker instead of fetching it, so nothing is loaded remotely.
  self.MonacoEnvironment = { getWorker: () => new editorWorker() };
  loader.config({ monaco });

  monaco.languages.register({ id: Z80_LANGUAGE });

  monaco.languages.setMonarchTokensProvider(Z80_LANGUAGE, {
    ignoreCase: true,
    mnemonics: MNEMONICS,
    directives: DIRECTIVES,
    registers: REGISTERS,
    tokenizer: {
      root: [
        [/;.*$/, "comment"],
        [/^[A-Za-z_.?@][\w.?@]*:?/, "type.identifier"],
        [/'(?:[^']|'')*'/, "string"],
        [/"[^"]*"/, "string"],
        // Hex in all three notations, then binary, octal and decimal.
        [/\$[0-9a-f]+\b/i, "number.hex"],
        [/0x[0-9a-f]+\b/i, "number.hex"],
        [/\d[0-9a-f]*h\b/i, "number.hex"],
        [/[01]+b\b/i, "number.binary"],
        [/[0-7]+[oq]\b/i, "number.octal"],
        [/\d+d?\b/, "number"],
        [/\$/, "variable.predefined"],
        [
          /[A-Za-z_][\w]*/,
          {
            cases: {
              "@mnemonics": "keyword",
              "@directives": "keyword.directive",
              "@registers": "variable.predefined",
              "@default": "identifier",
            },
          },
        ],
        [/[(),+\-*/&|^~<>=]/, "operator"],
      ],
    },
  });

  monaco.languages.setLanguageConfiguration(Z80_LANGUAGE, {
    comments: { lineComment: ";" },
    brackets: [["(", ")"]],
    autoClosingPairs: [
      { open: "(", close: ")" },
      { open: "'", close: "'" },
      { open: '"', close: '"' },
    ],
  });

  monaco.editor.defineTheme("z80-dark", {
    base: "vs-dark",
    inherit: true,
    rules: [
      { token: "keyword.directive", foreground: "c586c0" },
      { token: "type.identifier", foreground: "4ec9b0" },
      { token: "number.binary", foreground: "b5cea8" },
      { token: "number.octal", foreground: "b5cea8" },
    ],
    colors: {
      "editor.background": "#16181d",
    },
  });
}

export { monaco };
