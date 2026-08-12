//! The Windows backend: Win32 window control, the low-level input hooks, and the Fluent palette.

pub mod backend;
pub mod chrome;
pub mod hook;
pub mod keycode;
pub mod paths;
pub mod theme;
mod win32;

use std::sync::atomic::{AtomicBool, Ordering};

static SHOULD_QUIT: AtomicBool = AtomicBool::new(false);

/// Ask the run loop to exit (windows are restored on the way out).
pub fn request_quit() {
    SHOULD_QUIT.store(true, Ordering::SeqCst);
}

/// Whether a quit has been requested (polled by the run loop).
pub fn should_quit() -> bool {
    SHOULD_QUIT.load(Ordering::SeqCst)
}

pub use backend::WindowsBackend;
pub use chrome::D2DPainter;
pub use hook::Hooks;
