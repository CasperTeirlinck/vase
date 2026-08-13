//! The Windows backend: Win32 window control and the low-level input hooks.

pub mod backend;
pub mod chrome;
pub mod hook;
pub mod keycode;
pub mod paths;
pub mod tray;
mod win32;

use std::os::windows::io::IntoRawHandle;
use std::sync::atomic::{AtomicBool, Ordering};

use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Console::{SetStdHandle, STD_ERROR_HANDLE, STD_OUTPUT_HANDLE};

/// Posted to the daemon's thread when the icon worker has something to hand over. The run loop
/// redraws on it, so an icon appears as soon as it resolves rather than at the next unrelated redraw.
pub const ICONS_RESOLVED: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 2;

static SHOULD_QUIT: AtomicBool = AtomicBool::new(false);
static NEW_TAB: AtomicBool = AtomicBool::new(false);
static RELOAD_CONFIG: AtomicBool = AtomicBool::new(false);
static MODAL: AtomicBool = AtomicBool::new(false);

// Whether an overlay is taking keys, mirrored out of the daemon. The keyboard hook can be re-entered
// while the daemon is mid-operation and unable to answer, and a key belonging to an open overlay
// must not fall through to the focused app.
pub fn set_modal(open: bool) {
    MODAL.store(open, Ordering::SeqCst);
}
pub fn modal() -> bool {
    MODAL.load(Ordering::SeqCst)
}

/// Point stdout and stderr at the log file. vase is a windows-subsystem process, so it owns no
/// console and every diagnostic it prints would otherwise go nowhere. Each run starts a fresh log.
pub fn log_to_file() {
    let Some(path) = paths::log() else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let Ok(file) = std::fs::File::create(&path) else { return };
    // Into the raw handle, not a borrow: the standard handles outlive every scope here.
    let handle = HANDLE(file.into_raw_handle());
    let _ = unsafe { SetStdHandle(STD_OUTPUT_HANDLE, handle) };
    let _ = unsafe { SetStdHandle(STD_ERROR_HANDLE, handle) };
}

/// Ask the run loop to exit (windows are restored on the way out).
pub fn request_quit() {
    SHOULD_QUIT.store(true, Ordering::SeqCst);
}

/// Whether a quit has been requested (polled by the run loop).
pub fn should_quit() -> bool {
    SHOULD_QUIT.load(Ordering::SeqCst)
}

// Tray-menu actions run in the menu's own message dispatch, which holds no daemon reference; they
// set a flag the run loop drains into a daemon call.
pub fn request_new_tab() {
    NEW_TAB.store(true, Ordering::SeqCst);
}
pub fn take_new_tab() -> bool {
    NEW_TAB.swap(false, Ordering::SeqCst)
}
pub fn request_reload_config() {
    RELOAD_CONFIG.store(true, Ordering::SeqCst);
}
pub fn take_reload_config() -> bool {
    RELOAD_CONFIG.swap(false, Ordering::SeqCst)
}

pub use backend::WindowsBackend;
pub use chrome::D2DPainter;
pub use hook::Hooks;
pub use tray::Tray;
