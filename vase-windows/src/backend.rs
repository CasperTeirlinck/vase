//! The Win32 backend: everything vase asks of the OS, on Windows.

use std::collections::HashMap;

use windows::core::{BOOL, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS};
use windows::Win32::Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_APARTMENTTHREADED};
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION};
use windows::Win32::UI::Shell::{IVirtualDesktopManager, ShellExecuteW, VirtualDesktopManager};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetForegroundWindow, GetWindow, GetWindowRect, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindowVisible, IsZoomed, PostMessageW, SetForegroundWindow,
    SetWindowPos, ShowWindow, GW_OWNER, HWND_TOP, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SW_MINIMIZE, SW_RESTORE, SW_SHOW, WM_CLOSE, WS_CAPTION, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
};

use vase_core::backend::{Backend, Display, WindowInfo};
use vase_core::geometry::Rect;
use vase_core::tree::WindowId;

use crate::win32::{ex_style, from_wide, hwnd_of, id_of, rect_of, style, to_wide};

pub struct WindowsBackend {
    /// Last-seen info, so a window that has dropped off the live list still resolves.
    known: HashMap<WindowId, WindowInfo>,
    /// Tells "on another virtual desktop" apart from "closed".
    desktops: Option<IVirtualDesktopManager>,
}

impl WindowsBackend {
    pub fn new() -> Self {
        // Apartment-threaded: the daemon owns one thread and the shell interfaces below are STA.
        let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        let desktops = unsafe { CoCreateInstance(&VirtualDesktopManager, None, CLSCTX_ALL) }.ok();
        WindowsBackend { known: HashMap::new(), desktops }
    }

    /// Whether the window sits on the desktop the user is looking at. Fails open, so a failed query never drops a window.
    fn on_current_desktop(&self, hwnd: HWND) -> bool {
        let Some(vdm) = &self.desktops else { return true };
        unsafe { vdm.IsWindowOnCurrentVirtualDesktop(hwnd) }.map(|b| b.as_bool()).unwrap_or(true)
    }

    fn scan(&mut self, current_desktop_only: bool) -> Vec<WindowInfo> {
        let mut found: Vec<HWND> = Vec::new();
        // SAFETY: `collect_hwnd` only pushes into the Vec pointed at by lparam, which outlives the call.
        let _ = unsafe { EnumWindows(Some(collect_hwnd), LPARAM(&mut found as *mut Vec<HWND> as isize)) };
        let windows: Vec<WindowInfo> = found.into_iter().filter(|hwnd| !current_desktop_only || self.on_current_desktop(*hwnd)).filter_map(|hwnd| self.info(hwnd)).collect();
        // Merge, don't replace: a minimized window drops off the live list, but its info must stay
        // resolvable for restore; `forget` prunes on real removal.
        for w in &windows {
            self.known.insert(w.id, w.clone());
        }
        windows
    }

    fn info(&self, hwnd: HWND) -> Option<WindowInfo> {
        let mut pid = 0u32;
        let _ = unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
        Some(WindowInfo { id: id_of(hwnd), pid: pid as i32, app: process_name(pid).unwrap_or_default(), title: window_title(hwnd), frame: visible_frame(hwnd)?, layer: layer_of(hwnd) })
    }
}

impl Default for WindowsBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl Backend for WindowsBackend {
    fn displays(&self) -> Vec<Display> {
        let mut out: Vec<Display> = Vec::new();
        // SAFETY: `collect_monitor` only pushes into the Vec pointed at by dwdata, which outlives the call.
        let _ = unsafe { EnumDisplayMonitors(None, None, Some(collect_monitor), LPARAM(&mut out as *mut Vec<Display> as isize)) };
        out.sort_by(|a, b| a.bounds.x.total_cmp(&b.bounds.x).then(a.bounds.y.total_cmp(&b.bounds.y)));
        out
    }

    /// Frontmost first: EnumWindows walks the z-order.
    fn list_windows(&mut self) -> Vec<WindowInfo> {
        self.scan(true)
    }

    fn all_windows(&mut self) -> Vec<WindowInfo> {
        self.scan(false)
    }

    fn set_frame(&mut self, window: WindowId, frame: Rect) {
        let hwnd = hwnd_of(window);
        // Since Windows 10 a window's real rectangle is inset from `GetWindowRect` by an invisible
        // resize border. Place by the visible (DWM) frame, or neighbouring panes show a gap.
        let (Some(outer), Some(visible)) = (window_rect(hwnd), dwm_frame(hwnd)) else {
            let _ = unsafe { SetWindowPos(hwnd, None, frame.x as i32, frame.y as i32, frame.w as i32, frame.h as i32, SWP_NOACTIVATE | SWP_NOZORDER) };
            return;
        };
        let pad_x = visible.x - outer.x;
        let pad_y = visible.y - outer.y;
        let pad_w = outer.w - visible.w;
        let pad_h = outer.h - visible.h;
        let _ = unsafe { SetWindowPos(hwnd, None, (frame.x - pad_x) as i32, (frame.y - pad_y) as i32, (frame.w + pad_w) as i32, (frame.h + pad_h) as i32, SWP_NOACTIVATE | SWP_NOZORDER) };
    }

    /// Windows refuses `SetForegroundWindow` from a process that does not own the foreground, hence the input-queue attach.
    /// <https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setforegroundwindow>
    fn focus(&mut self, window: WindowId) {
        let hwnd = hwnd_of(window);
        unsafe {
            if IsIconic(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
            let foreground = GetForegroundWindow();
            let ours = GetCurrentThreadId();
            let theirs = GetWindowThreadProcessId(foreground, None);
            let attached = theirs != 0 && theirs != ours && AttachThreadInput(ours, theirs, true).as_bool();
            let _ = SetForegroundWindow(hwnd);
            if attached {
                let _ = AttachThreadInput(ours, theirs, false);
            }
        }
    }

    fn raise(&mut self, window: WindowId) {
        let hwnd = hwnd_of(window);
        // Z-order only: SWP_NOACTIVATE keeps keyboard focus where it is, and the trailing
        // FocusWindow effect of the same batch places it deliberately.
        let _ = unsafe { SetWindowPos(hwnd, Some(HWND_TOP), 0, 0, 0, 0, SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE) };
    }

    fn forget(&mut self, window: WindowId) {
        self.known.remove(&window);
    }

    fn title(&mut self, window: WindowId) -> Option<String> {
        let title = window_title(hwnd_of(window));
        (!title.is_empty()).then_some(title)
    }

    /// A top-level, un-owned application window. Tool windows, owned dialogs, and cloaked shell surfaces float.
    fn tileable(&mut self, info: &WindowInfo) -> Option<bool> {
        let hwnd = hwnd_of(info.id);
        if !unsafe { IsWindowVisible(hwnd) }.as_bool() {
            return Some(false);
        }
        let owned = !unsafe { GetWindow(hwnd, GW_OWNER) }.unwrap_or_default().is_invalid();
        let ex = ex_style(hwnd);
        Some(style(hwnd) & WS_CAPTION.0 != 0 && ex & WS_EX_TOOLWINDOW.0 == 0 && ex & WS_EX_NOACTIVATE.0 == 0 && !owned && !cloaked(hwnd))
    }

    fn minimized(&mut self, window: WindowId) -> Option<bool> {
        Some(unsafe { IsIconic(hwnd_of(window)) }.as_bool())
    }

    fn minimized_info(&mut self, info: &WindowInfo) -> Option<bool> {
        let m = unsafe { IsIconic(hwnd_of(info.id)) }.as_bool();
        self.known.entry(info.id).or_insert_with(|| info.clone());
        Some(m)
    }

    fn set_minimized(&mut self, window: WindowId, minimized: bool) {
        let _ = unsafe { ShowWindow(hwnd_of(window), if minimized { SW_MINIMIZE } else { SW_RESTORE }) };
    }

    /// Windows has no fullscreen flag: it means covering the whole monitor bounds, not just the work area.
    fn fullscreen(&mut self, info: &WindowInfo) -> Option<bool> {
        let hwnd = hwnd_of(info.id);
        if unsafe { IsZoomed(hwnd) }.as_bool() {
            return Some(false); // merely maximized: the taskbar is still visible
        }
        let frame = window_rect(hwnd)?;
        Some(self.displays().iter().any(|d| frame.x <= d.bounds.x && frame.y <= d.bounds.y && frame.w >= d.bounds.w && frame.h >= d.bounds.h))
    }

    fn close(&mut self, window: WindowId) {
        let _ = unsafe { PostMessageW(Some(hwnd_of(window)), WM_CLOSE, WPARAM(0), LPARAM(0)) };
    }

    /// Shortcut names from the Start Menu.
    fn launchable_apps(&self) -> Vec<String> {
        let mut apps: Vec<String> = crate::paths::start_menu_dirs().iter().flat_map(|dir| shortcuts(dir)).filter_map(|p| p.file_stem().and_then(|s| s.to_str()).map(str::to_string)).collect();
        apps.sort_by_key(|a| a.to_lowercase());
        apps.dedup();
        apps
    }

    fn launch(&self, app: &str) {
        let Some(path) = crate::paths::start_menu_dirs().iter().flat_map(|dir| shortcuts(dir)).find(|p| p.file_stem().and_then(|s| s.to_str()) == Some(app)) else {
            eprintln!("vase: no Start Menu entry for {app}");
            return;
        };
        let verb = to_wide("open");
        let target = to_wide(&path.to_string_lossy());
        // ShellExecuteW resolves the shortcut, so store apps and installer stubs still start.
        let _ = unsafe { ShellExecuteW(None, PCWSTR(verb.as_ptr()), PCWSTR(target.as_ptr()), None, None, SW_SHOW) };
    }
}

/// Every `.lnk` under `dir`, recursively.
pub fn shortcuts(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(shortcuts(&path));
        } else if path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("lnk")) {
            out.push(path);
        }
    }
    out
}

extern "system" fn collect_hwnd(hwnd: HWND, lparam: LPARAM) -> BOOL {
    // SAFETY: `scan` passes a pointer to a live Vec<HWND> and blocks until EnumWindows returns.
    let out = unsafe { &mut *(lparam.0 as *mut Vec<HWND>) };
    if unsafe { IsWindowVisible(hwnd) }.as_bool() {
        out.push(hwnd);
    }
    true.into()
}

extern "system" fn collect_monitor(monitor: HMONITOR, _: HDC, _: *mut RECT, lparam: LPARAM) -> BOOL {
    // SAFETY: `displays` passes a pointer to a live Vec<Display> and blocks until EnumDisplayMonitors returns.
    let out = unsafe { &mut *(lparam.0 as *mut Vec<Display>) };
    let mut info = MONITORINFOEXW { monitorInfo: MONITORINFO { cbSize: std::mem::size_of::<MONITORINFOEXW>() as u32, ..Default::default() }, ..Default::default() };
    if unsafe { GetMonitorInfoW(monitor, &mut info as *mut _ as *mut MONITORINFO) }.as_bool() {
        out.push(Display { id: device_id(&info.szDevice), bounds: rect_of(info.monitorInfo.rcMonitor), work_area: rect_of(info.monitorInfo.rcWork) });
    }
    true.into()
}

/// A stable id per display. `HMONITOR` is transient, so the adapter's device name is hashed instead.
fn device_id(device: &[u16; 32]) -> u32 {
    device.iter().take_while(|c| **c != 0).fold(2166136261u32, |h, c| (h ^ *c as u32).wrapping_mul(16777619))
}

fn window_title(hwnd: HWND) -> String {
    let len = unsafe { GetWindowTextLengthW(hwnd) };
    if len <= 0 {
        return String::new();
    }
    let mut buf = vec![0u16; len as usize + 1];
    let written = unsafe { GetWindowTextW(hwnd, &mut buf) };
    from_wide(&buf, written.max(0) as usize)
}

/// The executable's file stem.
fn process_name(pid: u32) -> Option<String> {
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    let mut buf = [0u16; 260];
    let mut len = buf.len() as u32;
    let ok = unsafe { QueryFullProcessImageNameW(handle, PROCESS_NAME_FORMAT(0), windows::core::PWSTR(buf.as_mut_ptr()), &mut len) }.is_ok();
    let _ = unsafe { windows::Win32::Foundation::CloseHandle(handle) };
    ok.then(|| std::path::PathBuf::from(from_wide(&buf, len as usize)).file_stem().and_then(|s| s.to_str()).map(str::to_string))?
}

fn window_rect(hwnd: HWND) -> Option<Rect> {
    let mut r = RECT::default();
    unsafe { GetWindowRect(hwnd, &mut r) }.ok()?;
    Some(rect_of(r))
}

/// The window's visible bounds, excluding the invisible resize border `GetWindowRect` includes.
fn dwm_frame(hwnd: HWND) -> Option<Rect> {
    let mut r = RECT::default();
    unsafe { DwmGetWindowAttribute(hwnd, DWMWA_EXTENDED_FRAME_BOUNDS, &mut r as *mut _ as *mut _, std::mem::size_of::<RECT>() as u32) }.ok()?;
    Some(rect_of(r))
}

fn visible_frame(hwnd: HWND) -> Option<Rect> {
    dwm_frame(hwnd).or_else(|| window_rect(hwnd))
}

/// Composited but not shown: suspended UWP apps, and windows on another virtual desktop.
fn cloaked(hwnd: HWND) -> bool {
    let mut value = 0u32;
    unsafe { DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED, &mut value as *mut _ as *mut _, 4) }.is_ok() && value != 0
}

/// The core's `manageable` keys off layer 0, so anything but a plain top-level app window reports nonzero.
fn layer_of(hwnd: HWND) -> i64 {
    let ex = ex_style(hwnd);
    let owned = !unsafe { GetWindow(hwnd, GW_OWNER) }.unwrap_or_default().is_invalid();
    let normal = style(hwnd) & WS_CAPTION.0 != 0 && ex & WS_EX_TOOLWINDOW.0 == 0 && !owned && !cloaked(hwnd);
    if normal {
        0
    } else {
        1
    }
}
