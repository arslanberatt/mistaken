//! Mistaken desktop application library entrypoint.

pub mod asr;
pub mod audio;
pub mod commands;
pub mod events;
pub mod state;

use std::sync::Arc;

use asr::loader::SherpaModelLoader;
use asr::manifest::DEVELOPMENT_MANIFEST;
use audio::microphone::CpalMicrophoneBackend;
use state::RuntimeManager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let manager = Arc::new(RuntimeManager::<tauri::Wry>::new(
        Arc::new(CpalMicrophoneBackend),
        audio::system::default_backend(),
        Arc::new(SherpaModelLoader::new(&DEVELOPMENT_MANIFEST)),
    ));
    let setup_manager = manager.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(move |app| {
            // Launch-time, metadata-only model presence report (existence
            // and byte size only; no hash, no load) so the top bar shows
            // an honest `Development ASR • Not release approved` /
            // `Model missing` state before the user ever presses Start.
            setup_manager.initialize_model_presence(app.handle());
            setup_manager.initialize_system_audio_presence(app.handle());
            Ok(())
        })
        .manage(manager)
        .invoke_handler(tauri::generate_handler![
            commands::runtime::get_runtime_snapshot,
            commands::runtime::list_microphones,
            commands::runtime::start_capture,
            commands::runtime::stop_capture,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Test-only support shared across module test suites. Exists so process-
/// global state (currently: the `MISTAKEN_MODEL_DIR` override env var) is
/// mutated at most once per test binary run instead of racing across
/// `cargo test`'s default multithreaded test execution.
#[cfg(test)]
pub(crate) mod test_support {
    use std::sync::Once;

    /// Fixed sentinel path used by every test that needs
    /// `resolve_model_dir` to succeed without touching a real resource
    /// directory. Tests that exercise this path always inject a fake
    /// [`crate::asr::AsrModelLoader`] that ignores the directory argument,
    /// so the path never needs to exist on disk.
    pub const MODEL_DIR_OVERRIDE: &str = "/tmp/mistaken-test-model-dir";

    static INIT: Once = Once::new();

    /// Sets `MISTAKEN_MODEL_DIR` to [`MODEL_DIR_OVERRIDE`] exactly once
    /// for the whole test binary. Safe to call from every test that needs
    /// it; only the first call has an effect.
    pub fn ensure_model_dir_env() {
        INIT.call_once(|| {
            // SAFETY: `Once` guarantees this runs exactly one time before
            // any caller proceeds, and no test removes or changes this
            // variable afterward.
            unsafe {
                std::env::set_var("MISTAKEN_MODEL_DIR", MODEL_DIR_OVERRIDE);
            }
        });
    }
}
