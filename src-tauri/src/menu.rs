//! The native macOS menu bar.
//!
//! The structure, the wording and the order come from the original's help
//! (File, Simulation, Rate, Tools, Options, Help), so an instruction written
//! for Z80 Simulator IDE — "Tools\Peripheral Devices" — leads to the same
//! place here. Items whose feature is not built yet are present but disabled,
//! rather than missing: a menu that is missing an entry is a menu you cannot
//! follow a lesson with.
//!
//! An Edit submenu is the one addition. macOS routes the clipboard shortcuts
//! through it, so without one, copy and paste stop working inside the editor.
//!
//! Choosing an item emits [`MENU_EVENT`] with the item's id. The window owns
//! the dispatch, because that is where the dialogs, the panels and the
//! simulation commands already live.

use tauri::menu::{CheckMenuItemBuilder, Menu, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder};
use tauri::{AppHandle, Emitter, Runtime};
use z80_system::machine::Options;
use z80_system::Rate;

/// The event the window listens on for menu choices. The payload is the id.
pub const MENU_EVENT: &str = "menu://action";

/// Items that mirror a boolean option, paired with the field they show.
const OPTION_CHECKS: [(&str, fn(&Options) -> bool); 4] = [
    ("options.halt-stops", |o| o.halt_stops_simulation),
    ("options.ff-defaults", |o| o.ff_power_on_defaults),
    ("options.unofficial", |o| o.enable_unofficial),
    ("options.prompt-in", |o| o.prompt_before_in),
];

/// Items that mirror the simulation rate.
const RATE_CHECKS: [(&str, Rate); 6] = [
    ("rate.step", Rate::StepByStep),
    ("rate.slow", Rate::Slow),
    ("rate.normal", Rate::Normal),
    ("rate.fast", Rate::Fast),
    ("rate.extremely-fast", Rate::ExtremelyFast),
    ("rate.ultimate", Rate::Ultimate),
];

pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    // Anything whose feature is not built yet.
    let pending = |id: &str, text: &str| MenuItemBuilder::with_id(id, text).enabled(false);

    let application = SubmenuBuilder::new(app, "Z80 Simulator IDE")
        .about(None)
        .separator()
        .services()
        .separator()
        .hide()
        .hide_others()
        .separator()
        .quit()
        .build()?;

    let file = SubmenuBuilder::new(app, "File")
        .item(&MenuItemBuilder::with_id("file.clear-memory", "Clear Memory").build(app)?)
        .item(
            &MenuItemBuilder::with_id("file.load-program", "Load Program")
                .accelerator("CmdOrCtrl+O")
                .build(app)?,
        )
        .item(&MenuItemBuilder::with_id("file.save-memory", "Save Memory").build(app)?)
        .separator()
        .item(
            &MenuItemBuilder::with_id("file.open-source", "Open Source…")
                .accelerator("CmdOrCtrl+Shift+O")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("file.save-source", "Save Source")
                .accelerator("CmdOrCtrl+S")
                .build(app)?,
        )
        .item(&MenuItemBuilder::with_id("file.save-source-as", "Save Source As…").build(app)?)
        .build()?;

    let edit = SubmenuBuilder::new(app, "Edit")
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;

    let simulation = SubmenuBuilder::new(app, "Simulation")
        .item(
            &MenuItemBuilder::with_id("simulation.start", "Start")
                .accelerator("F1")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("simulation.step", "Step")
                .accelerator("F2")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("simulation.stop", "Stop")
                .accelerator("F3")
                .build(app)?,
        )
        .separator()
        .item(&MenuItemBuilder::with_id("simulation.reset", "Reset").build(app)?)
        .build()?;

    let mut rate = SubmenuBuilder::new(app, "Rate");
    for (id, value) in RATE_CHECKS {
        let label = match value {
            Rate::StepByStep => "Step By Step",
            Rate::Slow => "Slow",
            Rate::Normal => "Normal",
            Rate::Fast => "Fast",
            Rate::ExtremelyFast => "Extremely Fast",
            Rate::Ultimate => "Ultimate",
        };
        rate = rate.item(
            &CheckMenuItemBuilder::with_id(id, label)
                .checked(value == Rate::default())
                .build(app)?,
        );
    }
    let rate = rate.build()?;

    let tools = SubmenuBuilder::new(app, "Tools")
        .item(&MenuItemBuilder::with_id("tools.memory-editor", "Memory Editor").build(app)?)
        .item(&MenuItemBuilder::with_id("tools.disassembler", "Disassembler").build(app)?)
        .item(&pending("tools.peripherals", "Peripheral Devices").build(app)?)
        .item(&MenuItemBuilder::with_id("tools.ports", "I/O Ports Editor").build(app)?)
        .item(&MenuItemBuilder::with_id("tools.memory-editor-2", "Memory Editor 2").build(app)?)
        .item(&MenuItemBuilder::with_id("tools.memory-editor-3", "Memory Editor 3").build(app)?)
        .item(&pending("tools.zx-spectrum", "ZX Spectrum ROM Interface").build(app)?)
        .item(&pending("tools.terminal", "Input/Output Terminal").build(app)?)
        .item(&pending(
            "tools.terminal-mapped",
            "Input/Output Terminal (Memory Mapped)",
        )
        .build(app)?)
        .item(&pending("tools.video", "Video Memory Screen").build(app)?)
        .separator()
        .item(
            &MenuItemBuilder::with_id("tools.assembler", "Assembler")
                .accelerator("F9")
                .build(app)?,
        )
        .item(&pending("tools.breakpoints", "Fast Debugger - Breakpoints Manager").build(app)?)
        .item(&pending("tools.basic", "BASIC Compiler").build(app)?)
        .item(&pending("tools.ieee754", "IEEE 754 Conversion Tool").build(app)?)
        .item(&MenuItemBuilder::with_id("tools.log-viewer", "Simulation Log Viewer").build(app)?)
        .item(&pending("tools.interactive-assembler", "Interactive Assembler Editor").build(app)?)
        .item(&pending("tools.watch", "Watch Variables").build(app)?)
        .item(&pending("tools.external-modules", "External Modules").build(app)?)
        .build()?;

    let options = SubmenuBuilder::new(app, "Options")
        .item(&pending("options.logging", "Enable Logging").build(app)?)
        .item(&pending("options.shortcuts", "Shortcuts Panel Configuration").build(app)?)
        .item(
            &CheckMenuItemBuilder::with_id("options.halt-stops", "HALT Stops Simulation")
                .checked(true)
                .build(app)?,
        )
        .item(
            &CheckMenuItemBuilder::with_id("options.ff-defaults", "FF Power On Defaults")
                .build(app)?,
        )
        .item(&pending("options.refresh-memory", "Refresh Memory Editor").build(app)?)
        .item(&pending("options.refresh-breakpoints", "Refresh Breakpoints Manager").build(app)?)
        .item(
            &CheckMenuItemBuilder::with_id("options.save-positions", "Save Positions")
                .checked(true)
                .enabled(false)
                .build(app)?,
        )
        .item(&pending("options.always-on-top", "Save Always On Top").build(app)?)
        .item(&pending("options.auto-start", "Auto Start Options").build(app)?)
        .separator()
        .item(&MenuItemBuilder::with_id("options.clock", "Change Clock Frequency").build(app)?)
        .item(&MenuItemBuilder::with_id("options.start-address", "Change Starting Address").build(app)?)
        .item(
            &MenuItemBuilder::with_id(
                "options.obj-offset",
                "Change Offset Address For OBJ Files Loading",
            )
            .build(app)?,
        )
        .item(
            &CheckMenuItemBuilder::with_id("options.unofficial", "Enable Unofficial Instructions")
                .checked(true)
                .build(app)?,
        )
        .item(
            &CheckMenuItemBuilder::with_id(
                "options.prompt-in",
                "Prompt For Value Before IN Instruction",
            )
            .build(app)?,
        )
        .item(&pending("options.io-logging", "Enable IN/OUT Instructions Logging").build(app)?)
        .item(&pending("options.confirmations", "Show Confirmation Boxes").build(app)?)
        .item(&pending("options.registers-arrangement", "Change Registers Arrangement").build(app)?)
        .item(&pending(
            "options.ultimate-interval",
            "Change Ultimate Rate Refresh Interval",
        )
        .build(app)?)
        .item(&pending("options.editor-setup", "Editor Setup").build(app)?)
        .item(&pending("options.theme", "Change Color Theme").build(app)?)
        .build()?;

    let help = SubmenuBuilder::new(app, "Help")
        .item(&pending("help.topics", "Help Topics").build(app)?)
        .item(&pending("help.basic-manual", "BASIC Compiler Reference Manual").build(app)?)
        .item(&pending("help.modules-manual", "External Modules Manual").build(app)?)
        .item(&pending("help.updates", "Check For Updates").build(app)?)
        .item(&pending("help.bug-report", "Bug Report Interface").build(app)?)
        .separator()
        .item(&PredefinedMenuItem::about(app, Some("About"), None)?)
        .item(&pending("help.license", "View License Information").build(app)?)
        .build()?;

    Menu::with_items(
        app,
        &[
            &application,
            &file,
            &edit,
            &simulation,
            &rate,
            &tools,
            &options,
            &help,
        ],
    )
}

/// Hand a menu choice to the window, which owns the dispatch.
pub fn dispatch<R: Runtime>(app: &AppHandle<R>, id: &str) {
    let _ = app.emit(MENU_EVENT, id);
}

/// Point the check marks at the simulation's actual state. The window calls
/// this whenever the options or the rate change, from wherever they changed:
/// the menu and the Options panel both edit the same values.
pub fn sync<R: Runtime>(app: &AppHandle<R>, options: &Options, rate: Rate) {
    let Some(menu) = app.menu() else { return };

    let set = |id: &str, checked: bool| {
        if let Some(item) = menu.get(id).and_then(|kind| kind.as_check_menuitem().cloned()) {
            let _ = item.set_checked(checked);
        }
    };

    for (id, read) in OPTION_CHECKS {
        set(id, read(options));
    }
    for (id, value) in RATE_CHECKS {
        set(id, value == rate);
    }
}
