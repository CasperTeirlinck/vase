pub mod ax;
pub mod cg;
pub mod dock;
pub mod event_tap;
pub mod keycode;
pub mod overlay;
pub mod paths;
pub mod status;

use std::sync::atomic::{AtomicBool, Ordering};

static SHOULD_QUIT: AtomicBool = AtomicBool::new(false);
static NEW_TAB: AtomicBool = AtomicBool::new(false);
static RELOAD_CONFIG: AtomicBool = AtomicBool::new(false);
static RESYNC: AtomicBool = AtomicBool::new(false);
static HELP: AtomicBool = AtomicBool::new(false);

/// Ask the daemon's run loop to exit (windows are restored on the way out).
pub fn request_quit() {
    SHOULD_QUIT.store(true, Ordering::SeqCst);
}

/// Whether a quit has been requested (polled by the run loop).
pub fn should_quit() -> bool {
    SHOULD_QUIT.load(Ordering::SeqCst)
}

// Menu-bar actions run on the AppKit side; they set a flag the run loop drains into a daemon call (the menu handler holds no daemon reference).
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
pub fn request_resync() {
    RESYNC.store(true, Ordering::SeqCst);
}
pub fn take_resync() -> bool {
    RESYNC.swap(false, Ordering::SeqCst)
}
pub fn request_help() {
    HELP.store(true, Ordering::SeqCst);
}
pub fn take_help() -> bool {
    HELP.swap(false, Ordering::SeqCst)
}

pub use ax::MacBackend;
pub use event_tap::EventTap;
pub use overlay::{nsapp_init, AppKitPainter};
