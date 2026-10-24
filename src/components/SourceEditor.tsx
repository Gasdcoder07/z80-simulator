import Editor, { type OnMount } from "@monaco-editor/react";
import { useEffect, useRef } from "react";

import { monaco, setupMonaco, Z80_LANGUAGE } from "../monaco-z80";
import { useSim } from "../store/sim";

setupMonaco();

interface Props {
  /** Set to jump the cursor to a line, as the problems list does. */
  revealLine: number | null;
}

export function SourceEditor({ revealLine }: Props) {
  const source = useSim((state) => state.source);
  const setSource = useSim((state) => state.setSource);
  const assembly = useSim((state) => state.assembly);
  const editorRef = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);

  const onMount: OnMount = (editor) => {
    editorRef.current = editor;
  };

  // Mark the assembler's errors in the gutter and under the offending text.
  useEffect(() => {
    const model = editorRef.current?.getModel();
    if (!model) return;

    const markers = (assembly?.errors ?? []).map((error) => ({
      severity: monaco.MarkerSeverity.Error,
      message: error.message,
      startLineNumber: error.line,
      endLineNumber: error.line,
      startColumn: 1,
      endColumn: model.getLineMaxColumn(Math.min(error.line, model.getLineCount())),
    }));
    monaco.editor.setModelMarkers(model, "assembler", markers);
  }, [assembly]);

  useEffect(() => {
    if (revealLine === null) return;
    const editor = editorRef.current;
    if (!editor) return;
    editor.revealLineInCenter(revealLine);
    editor.setPosition({ lineNumber: revealLine, column: 1 });
    editor.focus();
  }, [revealLine]);

  return (
    <Editor
      language={Z80_LANGUAGE}
      theme="z80-dark"
      value={source}
      onChange={(value) => setSource(value ?? "")}
      onMount={onMount}
      options={{
        fontSize: 13,
        fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
        minimap: { enabled: false },
        renderWhitespace: "none",
        scrollBeyondLastLine: false,
        tabSize: 8,
        insertSpaces: true,
        automaticLayout: true,
        glyphMargin: true,
      }}
    />
  );
}
