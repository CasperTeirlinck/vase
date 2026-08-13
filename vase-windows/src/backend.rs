//! The Win32 backend: everything vase asks of the OS, on Windows.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;

use windows::core::{BOOL, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS};
use windows::Win32::Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_APARTMENTTHREADED};
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::UI::Shell::{
    BHID_EnumItems, IEnumShellItems, IShellItem, IVirtualDesktopManager, SHCreateItemFromParsingName, ShellExecuteW, VirtualDesktopManager, SIGDN, SIGDN_NORMALDISPLAY, SIGDN_PARENTRELATIVEPARSING,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, EnumWindows, GetForegroundWindow, GetWindow, GetWindowPlacement, GetWindowRect, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindowVisible,
    IsZoomed, PostMessageW, SetForegroundWindow, SetWindowPos, ShowWindow, SystemParametersInfoW, GW_OWNER, HWND_TOP, SPI_GETFOREGROUNDLOCKTIMEOUT, SPI_SETFOREGROUNDLOCKTIMEOUT, SWP_NOACTIVATE,
    SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SW_MINIMIZE, SW_RESTORE, SW_SHOW, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, WINDOWPLACEMENT, WM_CLOSE, WS_CAPTION, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
};

use vase_core::backend::{Backend, Display, WindowInfo};
use vase_core::geometry::Rect;
use vase_core::tree::WindowId;

use windows::core::w;

/// The shell namespace folder that lists classic and packaged apps together.
const APPS_FOLDER: PCWSTR = w!("shell:AppsFolder");

use crate::win32::{ex_style, from_wide, hwnd_of, id_of, rect_of, style, to_wide};

pub struct WindowsBackend {
    /// Last-seen info, so a window that has dropped off the live list still resolves.
    known: HashMap<WindowId, WindowInfo>,
    /// Tells "on another virtual desktop" apart from "closed".
    desktops: Option<IVirtualDesktopManager>,
    /// Enumerated once: walking the shell's Applications folder costs a couple of hundred
    /// milliseconds, and apps do not come and go while vase runs.
    apps: RefCell<Option<Vec<(String, String)>>>,
}

impl WindowsBackend {
    pub fn new() -> Self {
        // Apartment-threaded: the daemon owns one thread and the shell interfaces below are STA.
        let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        let desktops = unsafe { CoCreateInstance(&VirtualDesktopManager, None, CLSCTX_ALL) }.ok();
        WindowsBackend { known: HashMap::new(), desktops, apps: RefCell::new(None) }
    }

    fn with_apps<T>(&self, read: impl FnOnce(&[(String, String)]) -> T) -> T {
        let mut cache = self.apps.borrow_mut();
        read(cache.get_or_insert_with(installed_apps))
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
        // A maximized window owns its monitor: it snaps straight back to the full work area whatever
        // `SetWindowPos` asks for. Un-maximize first, and read the padding below from the restored
        // window, whose invisible border is not the maximized one either.
        if unsafe { IsZoomed(hwnd) }.as_bool() {
            let _ = unsafe { ShowWindow(hwnd, SW_RESTORE) };
        }
        let scale = unsafe { GetDpiForWindow(hwnd) };
        place(hwnd, frame);
        // A window crossing onto a differently scaled monitor is rescaled by Windows as it arrives,
        // which lands it at half or double the size asked for. Placing again, now that it reports
        // the new scale, puts it exactly where it belongs.
        if unsafe { GetDpiForWindow(hwnd) } != scale {
            place(hwnd, frame);
        }
    }

    /// Windows only lets the process that owns the foreground hand it on, and vase is never that
    /// process: its own windows never activate, and it swallows the click that would have made it
    /// the last input event. So it borrows the right instead.
    /// <https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setforegroundwindow>
    fn focus(&mut self, window: WindowId) {
        let hwnd = hwnd_of(window);
        unsafe {
            if IsIconic(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
            let ours = GetCurrentThreadId();
            let outgoing = GetWindowThreadProcessId(GetForegroundWindow(), None);
            let incoming = GetWindowThreadProcessId(hwnd, None);
            // Both queues, not just the outgoing one: attaching to the incoming thread is what makes
            // the trailing `SetFocus` land, and attaching to the outgoing one is what makes the swap
            // legal at all.
            let attach = |thread: u32, on: bool| thread != 0 && thread != ours && AttachThreadInput(ours, thread, on).as_bool();
            let held_outgoing = attach(outgoing, true);
            let held_incoming = incoming != outgoing && attach(incoming, true);

            // Left alone, the shell defers a foreground change it considers unsolicited into a
            // taskbar flash. Zero for the duration of the swap, then put the user's value back.
            // No SPIF_SENDCHANGE: that broadcasts WM_SETTINGCHANGE to every top-level window and
            // blocks on their replies, which on this thread means dropped input events.
            // <https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-systemparametersinfow>
            let quietly = SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0);
            let mut lock_timeout = 0u32;
            let saved = SystemParametersInfoW(SPI_GETFOREGROUNDLOCKTIMEOUT, 0, Some(&mut lock_timeout as *mut u32 as *mut c_void), quietly).is_ok();
            if saved {
                let _ = SystemParametersInfoW(SPI_SETFOREGROUNDLOCKTIMEOUT, 0, None, quietly);
            }

            let _ = SetForegroundWindow(hwnd);
            let _ = BringWindowToTop(hwnd);
            let _ = SetFocus(Some(hwnd));

            if saved {
                let _ = SystemParametersInfoW(SPI_SETFOREGROUNDLOCKTIMEOUT, 0, Some(lock_timeout as usize as *mut c_void), quietly);
            }
            if held_incoming {
                attach(incoming, false);
            }
            if held_outgoing {
                attach(outgoing, false);
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
        // Covering a monitor is not enough on its own: only the main display reserves a strip for
        // the bar, so a window vase tiles onto any other one covers it exactly. What a window gives
        // up to take a screen is its title bar, and a window vase manages still has the caption that
        // made it manageable.
        if style(hwnd) & WS_CAPTION.0 != 0 {
            return Some(false);
        }
        let frame = window_rect(hwnd)?;
        Some(self.displays().iter().any(|d| frame.x <= d.bounds.x && frame.y <= d.bounds.y && frame.w >= d.bounds.w && frame.h >= d.bounds.h))
    }

    fn close(&mut self, window: WindowId) {
        let _ = unsafe { PostMessageW(Some(hwnd_of(window)), WM_CLOSE, WPARAM(0), LPARAM(0)) };
    }

    fn launchable_apps(&self) -> Vec<String> {
        let mut apps = self.with_apps(|apps| apps.iter().map(|(name, _)| name.clone()).collect::<Vec<String>>());
        apps.sort_by_key(|a| a.to_lowercase());
        apps.dedup();
        apps
    }

    fn launch(&self, app: &str) {
        let Some(target) = self.with_apps(|apps| apps.iter().find(|(name, _)| name == app).map(|(_, target)| target.clone())) else {
            eprintln!("vase: no installed app named {app}");
            return;
        };
        let target = to_wide(&target);
        // No verb: a shell path carries its own default action, and a packaged app has no "open".
        let _ = unsafe { ShellExecuteW(None, None, PCWSTR(target.as_ptr()), None, None, SW_SHOW) };
    }
}

/// Every app the shell can launch, as `(display name, shell path)`.
pub(crate) fn installed_apps() -> Vec<(String, String)> {
    // A packaged app has no `.lnk` anywhere on disk, so the Applications folder is the only listing
    // that carries both it and a classic app.
    let mut apps = Vec::new();
    unsafe {
        let folder: IShellItem = match SHCreateItemFromParsingName(APPS_FOLDER, None) {
            Ok(folder) => folder,
            Err(_) => return apps,
        };
        let items: IEnumShellItems = match folder.BindToHandler(None, &BHID_EnumItems) {
            Ok(items) => items,
            Err(_) => return apps,
        };
        loop {
            let mut fetched = [const { None }; 1];
            let mut count = 0;
            if items.Next(&mut fetched, Some(&mut count)).is_err() || count == 0 {
                return apps;
            }
            let Some(item) = fetched[0].take() else { return apps };
            // An item's parsing name is only meaningful inside this folder, so it is stored back
            // under it: `shell:AppsFolder\<id>` is what the shell can be asked to launch, for a
            // packaged app and a classic one alike.
            if let (Ok(name), Ok(id)) = (display_name(&item, SIGDN_NORMALDISPLAY), display_name(&item, SIGDN_PARENTRELATIVEPARSING)) {
                apps.push((name, format!("shell:AppsFolder\\{id}")));
            }
        }
    }
}

/// One of a shell item's names, copied out before the shell's own buffer is freed.
unsafe fn display_name(item: &IShellItem, kind: SIGDN) -> windows::core::Result<String> {
    let wide = unsafe { item.GetDisplayName(kind)? };
    let owned = unsafe { wide.to_string() };
    unsafe { CoTaskMemFree(Some(wide.0 as *const c_void)) };
    owned.map_err(Into::into)
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

pub fn exe_path(pid: u32) -> Option<std::path::PathBuf> {
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    let mut buf = [0u16; 260];
    let mut len = buf.len() as u32;
    let ok = unsafe { QueryFullProcessImageNameW(handle, PROCESS_NAME_FORMAT(0), windows::core::PWSTR(buf.as_mut_ptr()), &mut len) }.is_ok();
    let _ = unsafe { windows::Win32::Foundation::CloseHandle(handle) };
    ok.then(|| std::path::PathBuf::from(from_wide(&buf, len as usize)))
}

/// The executable's file stem.
fn process_name(pid: u32) -> Option<String> {
    exe_path(pid)?.file_stem().and_then(|s| s.to_str()).map(str::to_string)
}

/// Move a window so its visible frame is `frame`.
fn place(hwnd: HWND, frame: Rect) {
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
    // A minimized window's rectangle is its icon's: a few dozen pixels across, parked off-screen.
    // Small enough that the core would take it for a popup and never adopt it, so what counts is
    // the frame it will come back to.
    if unsafe { IsIconic(hwnd) }.as_bool() {
        return restored_frame(hwnd);
    }
    dwm_frame(hwnd).or_else(|| window_rect(hwnd))
}

fn restored_frame(hwnd: HWND) -> Option<Rect> {
    let mut placement = WINDOWPLACEMENT { length: std::mem::size_of::<WINDOWPLACEMENT>() as u32, ..Default::default() };
    unsafe { GetWindowPlacement(hwnd, &mut placement) }.ok()?;
    Some(rect_of(placement.rcNormalPosition))
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
