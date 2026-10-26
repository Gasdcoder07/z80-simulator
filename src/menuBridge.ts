import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import { loadProgram, openSource, saveSource } from "./fileActions";
import { useSim } from "./store/sim";
import type { Options, Rate } from "./types";

/** The event the native menu emits, carrying the chosen item's id. */
const MENU_EVENT = "menu://action";

/** Menu ids that only change the simulation rate. */
const RATES: Record<string, Rate> = {
  "rate.step": "stepByStep",
  "rate.slow": "slow",
  "rate.normal": "normal",
  "rate.fast": "fast",
  "rate.extremely-fast": "extremelyFast",
  "rate.ultimate": "ultimate",
};

/** Menu ids that toggle one boolean option. */
const OPTION_TOGGLES: Record<string, keyof Options> = {
  "options.halt-stops": "haltStopsSimulation",
  "options.ff-defaults": "ffPowerOnDefaults",
  "options.unofficial": "enableUnofficial",
  "options.prompt-in": "promptBeforeIn",
};

/** What the window has to do itself, because the menu cannot reach into it. */
export interface MenuTargets {
  showMemoryTab: (index: number) => void;
  showBottomTab: (tab: string) => void;
  askMemoryRange: () => void;
  focusSource: () => void;
}

/**
 * Wire the native menu to the window. Every item the original has is in the
 * menu; the ones whose feature is not built yet arrive here disabled and
 * never fire.
 */
export function listenToMenu(targets: MenuTargets): Promise<() => void> {
  return listen<string>(MENU_EVENT, async (event) => {
    const id = event.payload;
    const state = useSim.getState();

    if (id in RATES) {
      await state.setRate(RATES[id]);
      return;
    }

    if (id in OPTION_TOGGLES) {
      const options = state.snapshot?.options;
      if (!options) return;
      const key = OPTION_TOGGLES[id];
      await state.setOptions({ ...options, [key]: !options[key] });
      return;
    }

    switch (id) {
      // ---- File
      case "file.clear-memory":
        await state.send({ type: "clearMemory" });
        state.setStatus("Memory cleared.");
        return;
      case "file.load-program":
        return void loadProgram();
      case "file.save-memory":
        return targets.askMemoryRange();
      case "file.open-source":
        return void openSource();
      case "file.save-source":
        return void saveSource(false);
      case "file.save-source-as":
        return void saveSource(true);

      // ---- Simulation
      case "simulation.start":
        return void state.send({ type: "start" });
      case "simulation.step":
        return void state.send({ type: "step" });
      case "simulation.stop":
        return void state.send({ type: "stop" });
      case "simulation.reset":
        return void state.send({ type: "reset" });

      // ---- Tools
      case "tools.memory-editor":
        return targets.showMemoryTab(0);
      case "tools.memory-editor-2":
        return targets.showMemoryTab(1);
      case "tools.memory-editor-3":
        return targets.showMemoryTab(2);
      case "tools.ports":
        return targets.showBottomTab("ports");
      case "tools.log-viewer":
        return targets.showBottomTab("trace");
      case "tools.assembler":
        return void state.assemble(true);
      case "tools.disassembler":
        // The disassembly sits in the debug pane and is always on screen.
        state.setStatus("The disassembly is in the CPU pane, following the program counter.");
        return;

      // ---- Options that open the panel rather than toggling
      case "options.clock":
      case "options.start-address":
      case "options.obj-offset":
        return targets.showBottomTab("options");
    }
  });
}

/** Keep the menu's check marks pointed at the simulation's actual state. */
export async function syncMenu(options: Options, rate: Rate) {
  try {
    await invoke("sync_menu", { options, rate });
  } catch {
    // A menu that is briefly out of step is not worth interrupting the user.
  }
}
