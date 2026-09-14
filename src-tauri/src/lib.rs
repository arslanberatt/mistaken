//! Mistaken desktop application library entrypoint.

pub mod audio;
pub mod commands;
pub mod events;
pub mod state;

use std::sync::Mutex;

use state::RuntimeState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(Mutex::new(RuntimeState::new()))
        .invoke_handler(tauri::generate_handler![
            commands::runtime::get_runtime_snapshot,
            commands::runtime::list_microphones,
            commands::runtime::start_capture,
            commands::runtime::stop_capture,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
