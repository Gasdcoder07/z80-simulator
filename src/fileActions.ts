import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { readFile, readTextFile, writeTextFile } from "@tauri-apps/plugin-fs";

import { useSim } from "./store/sim";

/**
 * The file commands, shared by the toolbar and the menu bar so that both
 * routes do exactly the same thing. They read the store directly rather than
 * taking props, because a menu handler has no component to hang them on.
 */

const ASM_FILTERS = [{ name: "Assembler source", extensions: ["asm", "z80", "s", "txt"] }];
const IMAGE_FILTERS = [{ name: "Program image", extensions: ["hex", "obj", "bin", "com", "rom"] }];
const HEX_FILTERS = [{ name: "Intel HEX", extensions: ["hex"] }];

export async function openSource() {
  const { setSource, setPath, markSaved, setStatus } = useSim.getState();
  const chosen = await open({ multiple: false, filters: ASM_FILTERS });
  if (typeof chosen !== "string") return;
  setSource(await readTextFile(chosen));
  setPath(chosen);
  markSaved();
  setStatus(`Opened ${chosen}`);
}

export async function saveSource(forceDialog: boolean) {
  const { source, path, setPath, markSaved, setStatus } = useSim.getState();
  let target = path;
  if (forceDialog || !path.includes("/")) {
    const chosen = await save({ defaultPath: path, filters: ASM_FILTERS });
    if (!chosen) return;
    target = chosen;
    setPath(chosen);
  }
  await writeTextFile(target, source);
  markSaved();
  setStatus(`Saved ${target}`);
}

/** "Load Program" in the original: an Intel HEX or a raw image. */
export async function loadProgram() {
  const { snapshot, loadImage } = useSim.getState();
  const chosen = await open({ multiple: false, filters: IMAGE_FILTERS });
  if (typeof chosen !== "string") return;
  const bytes = await readFile(chosen);
  await loadImage(Array.from(bytes), snapshot?.options.objLoadOffset ?? 0);
}

/** Assemble and write the Intel HEX of the result. */
export async function saveHex() {
  const { assemble, path, setStatus } = useSim.getState();
  const result = await assemble(false);
  if (!result?.ok) {
    setStatus("Fix the assembly errors before saving a HEX file.");
    return;
  }
  const chosen = await save({
    defaultPath: path.replace(/\.[^.]+$/, "") + ".hex",
    filters: HEX_FILTERS,
  });
  if (!chosen) return;
  await writeTextFile(chosen, result.hex);
  setStatus(`Saved ${chosen}`);
}

/**
 * "Save Memory": a range of the simulated memory as Intel HEX. The range
 * comes from the dialog, not from what was assembled, so a program poked in
 * by hand saves too.
 */
export async function saveMemoryRange(from: number, to: number) {
  const { path, setStatus } = useSim.getState();
  try {
    const hex = await invoke<string>("save_memory", { from, to });
    const chosen = await save({
      defaultPath: path.replace(/\.[^.]+$/, "") + "-memory.hex",
      filters: HEX_FILTERS,
    });
    if (!chosen) return;
    await writeTextFile(chosen, hex);
    setStatus(`Saved ${chosen}`);
  } catch (error) {
    setStatus(String(error));
  }
}
