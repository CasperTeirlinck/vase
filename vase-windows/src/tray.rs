//! The notification-area icon.

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::UI::Shell::{ShellExecuteW, Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow, GetCursorPos, MessageBoxW, RegisterClassW, SetForegroundWindow, TrackPopupMenu, HWND_MESSAGE,
    MB_ICONINFORMATION, MB_OK, MF_SEPARATOR, MF_STRING, SW_SHOW, TPM_RIGHTBUTTON, WM_APP, WM_COMMAND, WM_LBUTTONUP, WM_RBUTTONUP, WNDCLASSW,
};

use crate::win32::to_wide;

mod icon;

/// Where the shell delivers the icon's mouse events. Any value from `WM_APP` up is ours to pick.
const CALLBACK: u32 = WM_APP + 1;
const CLASS: PCWSTR = w!("vase_tray");

const ABOUT: usize = 1;
const NEW_TAB: usize = 2;
const RESYNC: usize = 6;
const HELP: usize = 7;
const RELOAD_CONFIG: usize = 3;
const SETTINGS: usize = 4;
const QUIT: usize = 5;

/// The installed tray icon; dropping this removes it. The shell leaves a dead icon behind if the
/// process exits without it.
pub struct Tray {
    data: NOTIFYICONDATAW,
}

impl Drop for Tray {
    fn drop(&mut self) {
        let _ = unsafe { Shell_NotifyIconW(NIM_DELETE, &self.data) };
        let _ = unsafe { DestroyWindow(self.data.hWnd) };
    }
}

impl Tray {
    /// Add the icon, owned by a message-only window on the calling thread. The caller must pump that
    /// thread's messages, which the daemon's run loop already does.
    pub fn install() -> Option<Tray> {
        unsafe {
            let class = WNDCLASSW { lpfnWndProc: Some(wndproc), lpszClassName: CLASS, ..Default::default() };
            RegisterClassW(&class);
            let hwnd = CreateWindowExW(Default::default(), CLASS, PCWSTR::null(), Default::default(), 0, 0, 0, 0, Some(HWND_MESSAGE), None, None, None).ok()?;
            let mut data = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: hwnd,
                uID: 1,
                uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
                uCallbackMessage: CALLBACK,
                hIcon: icon::vase_mark_icon().unwrap_or_default(),
                ..Default::default()
            };
            for (slot, c) in data.szTip.iter_mut().zip("vase".encode_utf16()) {
                *slot = c;
            }
            Shell_NotifyIconW(NIM_ADD, &data).ok().ok()?;
            Some(Tray { data })
        }
    }
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        CALLBACK if matches!(lparam.0 as u32, WM_LBUTTONUP | WM_RBUTTONUP) => {
            show_menu(hwnd);
            LRESULT(0)
        }
        WM_COMMAND => {
            match wparam.0 & 0xffff {
                ABOUT => about(),
                HELP => crate::request_help(),
                NEW_TAB => crate::request_new_tab(),
                RESYNC => crate::request_resync(),
                RELOAD_CONFIG => crate::request_reload_config(),
                SETTINGS => open_settings(),
                QUIT => crate::request_quit(),
                _ => {}
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn show_menu(hwnd: HWND) {
    unsafe {
        let Ok(menu) = CreatePopupMenu() else { return };
        let item = |id: usize, label: PCWSTR| {
            let _ = AppendMenuW(menu, MF_STRING, id, label);
        };
        let separator = || {
            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        };
        item(ABOUT, w!("About vase"));
        separator();
        item(HELP, w!("Keyboard shortcuts"));
        item(NEW_TAB, w!("New tab"));
        item(RESYNC, w!("Resync windows"));
        item(RELOAD_CONFIG, w!("Reload config"));
        separator();
        item(SETTINGS, w!("Settings…"));
        separator();
        item(QUIT, w!("Quit vase"));

        let mut at = POINT::default();
        let _ = GetCursorPos(&mut at);
        // Without the foreground handoff the menu survives a click elsewhere on screen.
        let _ = SetForegroundWindow(hwnd);
        let _ = TrackPopupMenu(menu, TPM_RIGHTBUTTON, at.x, at.y, None, hwnd, None);
        let _ = DestroyMenu(menu);
    }
}

fn about() {
    let text = to_wide(&format!("version {}\n\nA keyboard-driven manual tiling window manager.", env!("CARGO_PKG_VERSION")));
    unsafe { MessageBoxW(None, PCWSTR(text.as_ptr()), w!("vase"), MB_OK | MB_ICONINFORMATION) };
}

/// Open the config file in whatever the user edits `.toml` with.
fn open_settings() {
    let Some(path) = crate::paths::ensure_config() else { return };
    let target = to_wide(&path.to_string_lossy());
    unsafe { ShellExecuteW(None, w!("open"), PCWSTR(target.as_ptr()), None, None, SW_SHOW) };
}
