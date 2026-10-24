//! A Z80 simulator IDE: assembler, disassembler, debugger and simulated
//! hardware, in one window.

mod commands;
mod sim;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        // Remembers window size and position between runs, which is the
        // "Save Positions" option of the original.
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .setup(|app| {
            // The simulation thread needs a handle so it can push snapshots
            // to the window without being asked.
            let handle = sim::spawn(app.handle().clone());
            app.manage(handle);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::assemble_source,
            commands::sim_command,
            commands::sim_snapshot,
            commands::load_image,
        ])
        .run(tauri::generate_context!())
        .expect("the application failed to start");
}
