//! COM apartment lifetime, scoped to the adapter's own capture thread only
//! (spec section 11 "COM and handle lifetime rules"). Never constructed on a
//! caller/Tauri-owned thread: Tauri and the application own their threads'
//! apartments, and this crate never calls `CoInitializeEx`/`CoUninitialize`
//! anywhere but the dedicated thread it spawns in `capture.rs`.

#![cfg(windows)]

use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};

/// RAII guard for one thread's COM apartment. `CoUninitialize` runs in
/// `Drop`, so callers declare this guard before any COM interface it will
/// outlive and let normal drop order (reverse of declaration) release every
/// interface first.
///
/// Deliberately `!Send`/`!Sync` in spirit — never moved to another thread —
/// even though the type itself carries no data; the invariant is enforced by
/// construction discipline (only ever created inside the adapter's own
/// spawned thread closure) rather than by a marker type, matching the crate's
/// stated safety rule for COM scoping.
pub(crate) struct ComScope {
    _private: (),
}

impl ComScope {
    /// Initializes the calling thread's COM apartment as MTA. Must be called
    /// exactly once per adapter capture thread, on that thread, before any
    /// other COM call on it.
    ///
    /// # Safety invariant
    /// `CoInitializeEx` must be paired with exactly one `CoUninitialize` on
    /// the same OS thread after every COM interface obtained during this
    /// scope has been released. This type's `Drop` provides that pairing;
    /// callers must not call `CoInitializeEx`/`CoUninitialize` directly.
    pub fn initialize_mta() -> windows::core::Result<Self> {
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok()?;
        Ok(Self { _private: () })
    }
}

impl Drop for ComScope {
    fn drop(&mut self) {
        // Safety: paired 1:1 with the `CoInitializeEx` call in
        // `initialize_mta`, on the same thread, after every COM interface
        // obtained during this scope's lifetime has already been dropped by
        // this point in normal (reverse) drop order.
        unsafe { CoUninitialize() };
    }
}
