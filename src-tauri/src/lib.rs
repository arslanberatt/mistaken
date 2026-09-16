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

/// Hand-written FFI to the platform's already-linked system library for
/// `SIGINT`/`SIGTERM` (Unix) or console control events (Windows). Spec 10
/// requires a real termination-signal handler that runs the single
/// idempotent `shutdown()` before exit, but may not add a Cargo
/// dependency (`ctrlc`/`signal-hook` were considered and rejected solely
/// for that reason) — raw `extern` bindings avoid one entirely.
mod termination_signal {
    use std::sync::atomic::{AtomicBool, Ordering};

    static REQUESTED: AtomicBool = AtomicBool::new(false);

    /// Installs the platform handler. Both handler bodies are
    /// async-signal-safe: an atomic store and nothing else.
    pub fn install() {
        imp::install();
    }

    /// Polled by a dedicated watcher thread rather than acted on directly
    /// inside the signal handler, since the handler itself must stay
    /// async-signal-safe (no locks, no allocation, no native audio calls).
    pub fn requested() -> bool {
        REQUESTED.load(Ordering::SeqCst)
    }

    fn mark_requested() {
        REQUESTED.store(true, Ordering::SeqCst);
    }

    #[cfg(unix)]
    mod imp {
        use super::mark_requested;

        const SIGINT: i32 = 2;
        const SIGTERM: i32 = 15;

        unsafe extern "C" {
            fn signal(signum: i32, handler: usize) -> usize;
        }

        extern "C" fn on_signal(_signum: i32) {
            mark_requested();
        }

        pub fn install() {
            // SAFETY: `on_signal` only performs an atomic store, which is
            // async-signal-safe; `signal` is the standard POSIX libc entry
            // point already linked into every Unix binary.
            unsafe {
                signal(SIGINT, on_signal as *const () as usize);
                signal(SIGTERM, on_signal as *const () as usize);
            }
        }
    }

    #[cfg(windows)]
    mod imp {
        use super::mark_requested;

        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn SetConsoleCtrlHandler(handler: usize, add: i32) -> i32;
        }

        // CTRL_C_EVENT, CTRL_CLOSE_EVENT, and friends all route here; every
        // one of them means "the console/process is going away".
        unsafe extern "system" fn on_ctrl_event(_ctrl_type: u32) -> i32 {
            mark_requested();
            1 // TRUE: handled, do not fall through to the default handler.
        }

        pub fn install() {
            // SAFETY: `on_ctrl_event` only performs an atomic store, which
            // is safe to call from this callback; `kernel32` is already
            // linked into every Windows binary.
            unsafe {
                SetConsoleCtrlHandler(on_ctrl_event as *const () as usize, 1);
            }
        }
    }

    #[cfg(not(any(unix, windows)))]
    mod imp {
        pub fn install() {}
    }
}

/// Spawns the one background thread that polls for a delivered
/// termination signal and runs the single idempotent `shutdown()` before
/// exiting the process. The signal handler itself cannot safely call into
/// native audio/lock code, so it only sets a flag; this thread does the
/// real work.
fn spawn_termination_watcher(manager: Arc<RuntimeManager>) {
    termination_signal::install();
    std::thread::Builder::new()
        .name("mistaken-termination-watcher".into())
        .spawn(move || loop {
            if termination_signal::requested() {
                manager.shutdown();
                std::process::exit(0);
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        })
        .expect("failed to spawn termination-signal watcher thread");
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let manager = Arc::new(RuntimeManager::<tauri::Wry>::new(
        Arc::new(CpalMicrophoneBackend),
        audio::system::default_backend(),
        Arc::new(SherpaModelLoader::new(&DEVELOPMENT_MANIFEST)),
    ));
    let setup_manager = manager.clone();
    let window_event_manager = manager.clone();
    let run_event_manager = manager.clone();

    spawn_termination_watcher(manager.clone());

    let app = tauri::Builder::default()
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
        // Entry point 1 of the single idempotent shutdown path (Spec 10):
        // the main window's close request. Never waits for a React
        // listener, an IPC response, or a webview state.
        .on_window_event(move |_window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                window_event_manager.shutdown();
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    // Entry points 2 and 3: the application's own exit sequence
    // (`ExitRequested`, fired once when the app is about to quit) and the
    // event loop's final `Exit`. `shutdown()` is idempotent, so calling it
    // from more than one of these three entry points on the same quit is
    // safe by design.
    app.run(move |_app_handle, event| match event {
        tauri::RunEvent::ExitRequested { .. } => {
            run_event_manager.shutdown();
        }
        tauri::RunEvent::Exit => {
            run_event_manager.shutdown();
        }
        _ => {}
    });
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
