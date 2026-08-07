//! Accessibility: resolve a window's AXUIElement and set its frame.

use std::collections::HashMap;

use accessibility_sys::{
    kAXErrorSuccess, kAXFrontmostAttribute, kAXPositionAttribute, kAXRaiseAction,
    kAXSizeAttribute, kAXValueTypeCGPoint, kAXValueTypeCGSize, kAXWindowsAttribute,
    AXUIElementCopyAttributeValue, AXUIElementCreateApplication, AXUIElementPerformAction,
    AXUIElementRef, AXUIElementSetAttributeValue, AXValueCreate,
};

// Private HIServices function mapping an AXUIElement to its CoreGraphics window
// id. Read-only and SIP-compatible (no injection, no SIP disable) — the same
// window-identity primitive pure-Accessibility WMs like AeroSpace rely on. Not
// exposed by accessibility-sys, so we declare it; it links via ApplicationServices
// (already pulled in by accessibility-sys).
extern "C" {
    fn _AXUIElementGetWindow(element: AXUIElementRef, out: *mut u32) -> i32;
}

// Carbon Process Manager + private SkyLight functions for focusing a *specific*
// window across displays/Spaces — the one thing pure Accessibility can't do for
// an app with windows on multiple displays (it always focuses the app's window
// on the active display). Same technique yabai uses. GetProcessForPID links via
// ApplicationServices; the SLPS* symbols via SkyLight (see build.rs).
#[repr(C)]
struct ProcessSerialNumber {
    high: u32,
    low: u32,
}
const K_CPS_USER_GENERATED: u32 = 0x2;
extern "C" {
    fn GetProcessForPID(pid: i32, psn: *mut ProcessSerialNumber) -> i32;
    fn _SLPSSetFrontProcessWithOptions(psn: *const ProcessSerialNumber, wid: u32, mode: u32) -> i32;
    fn SLPSPostEventRecordTo(psn: *const ProcessSerialNumber, bytes: *const u8) -> i32;
}

/// Give a specific window keyboard focus regardless of which display/Space it's
/// on: make its process front *with this window*, post the two "make key" event
/// records SkyLight expects (byte layout reverse-engineered by yabai), then raise
/// it. Falls back cleanly (does nothing) if the process id can't be resolved.
unsafe fn focus_window_skylight(pid: i32, window_id: u32, el: AXUIElementRef) {
    let mut psn = ProcessSerialNumber { high: 0, low: 0 };
    if GetProcessForPID(pid, &mut psn) != 0 {
        return;
    }
    _SLPSSetFrontProcessWithOptions(&psn, window_id, K_CPS_USER_GENERATED);
    // Two synthetic event records carrying the window id at offset 0x3c; the
    // second differs only in the 0x08 tag (0x01 then 0x02).
    let mut bytes = [0u8; 0xf8];
    bytes[0x04] = 0xf8;
    bytes[0x08] = 0x01;
    bytes[0x3a] = 0x10;
    bytes[0x3c..0x40].copy_from_slice(&window_id.to_ne_bytes());
    bytes[0x20..0x30].fill(0xff);
    SLPSPostEventRecordTo(&psn, bytes.as_ptr());
    bytes[0x08] = 0x02;
    SLPSPostEventRecordTo(&psn, bytes.as_ptr());

    let raise = CFString::from_static_string(kAXRaiseAction);
    AXUIElementPerformAction(el, raise.as_concrete_TypeRef());
}
use core_foundation::array::CFArray;
use core_foundation::base::{CFRelease, CFRetain, CFType, CFTypeRef, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::string::CFString;
use core_graphics::geometry::{CGPoint, CGSize};
use vase_core::backend::{Backend, WindowInfo};
use vase_core::geometry::Rect;
use vase_core::tree::WindowId;

/// Owns a retained AXUIElement and releases it on drop.
struct AxElement(AXUIElementRef);
impl Drop for AxElement {
    fn drop(&mut self) {
        unsafe { CFRelease(self.0 as CFTypeRef) }
    }
}

pub struct MacBackend {
    handles: HashMap<WindowId, AxElement>,
    known: HashMap<WindowId, WindowInfo>,
}

impl MacBackend {
    pub fn new() -> Self {
        MacBackend { handles: HashMap::new(), known: HashMap::new() }
    }

    /// Find and retain the AXUIElement for `info`, matching by CoreGraphics
    /// window id (co-located same-app windows share a frame, so geometry can't
    /// distinguish them — identity can).
    fn resolve(&mut self, info: &WindowInfo) -> Option<AXUIElementRef> {
        if let Some(h) = self.handles.get(&info.id) {
            return Some(h.0);
        }
        unsafe {
            let app = AXUIElementCreateApplication(info.pid);
            if app.is_null() {
                return None;
            }
            let attr = CFString::from_static_string(kAXWindowsAttribute);
            let mut value: CFTypeRef = std::ptr::null();
            let err = AXUIElementCopyAttributeValue(app, attr.as_concrete_TypeRef(), &mut value);
            CFRelease(app as CFTypeRef);
            if err != kAXErrorSuccess || value.is_null() {
                return None;
            }
            let windows: CFArray<CFType> = CFArray::wrap_under_create_rule(value as _);
            for w in windows.iter() {
                let el = w.as_CFTypeRef() as AXUIElementRef;
                let mut cg_id: u32 = 0;
                if _AXUIElementGetWindow(el, &mut cg_id) == kAXErrorSuccess
                    && cg_id as u64 == info.id.0
                {
                    let retained = CFRetain(el as CFTypeRef) as AXUIElementRef;
                    self.handles.insert(info.id, AxElement(retained));
                    return Some(retained);
                }
            }
        }
        None
    }

    /// The window's current title via Accessibility (`AXTitle`) — live and
    /// permission-light, unlike `kCGWindowName` (needs Screen Recording, stale).
    /// `None` if it can't be resolved; `Some("")` for a genuinely title-less window.
    pub fn title(&mut self, window: WindowId) -> Option<String> {
        let info = self.known.get(&window).cloned()?;
        let el = self.resolve(&info)?;
        unsafe {
            let attr = CFString::from_static_string("AXTitle");
            let mut value: CFTypeRef = std::ptr::null();
            let err = AXUIElementCopyAttributeValue(el, attr.as_concrete_TypeRef(), &mut value);
            if err != kAXErrorSuccess || value.is_null() {
                return None;
            }
            Some(CFString::wrap_under_create_rule(value as _).to_string())
        }
    }

    /// The window's Accessibility subrole (e.g. `AXStandardWindow`, `AXDialog`,
    /// `AXUnknown`), read straight from `info` — used to skip transient popups
    /// (download bubbles, panels) that shouldn't be tiled. `None` if unreadable.
    pub fn subrole_info(&mut self, info: &WindowInfo) -> Option<String> {
        let el = self.resolve(info)?;
        unsafe {
            let attr = CFString::from_static_string("AXSubrole");
            let mut value: CFTypeRef = std::ptr::null();
            let err = AXUIElementCopyAttributeValue(el, attr.as_concrete_TypeRef(), &mut value);
            if err != kAXErrorSuccess || value.is_null() {
                return None;
            }
            Some(CFString::wrap_under_create_rule(value as _).to_string())
        }
    }

    /// Close a window by pressing its AX close button (the reconcile poll then
    /// removes it from the model). No-op if the window or its button is gone.
    pub fn close(&mut self, window: WindowId) {
        let Some(info) = self.known.get(&window).cloned() else { return };
        let Some(el) = self.resolve(&info) else { return };
        unsafe {
            let attr = CFString::from_static_string("AXCloseButton");
            let mut button: CFTypeRef = std::ptr::null();
            let err = AXUIElementCopyAttributeValue(el, attr.as_concrete_TypeRef(), &mut button);
            if err == kAXErrorSuccess && !button.is_null() {
                let press = CFString::from_static_string("AXPress");
                AXUIElementPerformAction(button as AXUIElementRef, press.as_concrete_TypeRef());
                CFRelease(button);
            }
        }
    }

    /// `Some(true)` if the window is minimized to the Dock, `Some(false)` if it
    /// exists and isn't, `None` if it can't be read (treated as gone/closed).
    pub fn minimized(&mut self, window: WindowId) -> Option<bool> {
        let info = self.known.get(&window).cloned()?;
        let el = self.resolve(&info)?;
        unsafe { read_minimized(el) }
    }

    /// Like `minimized`, but for a window not yet tracked (startup discovery of
    /// already-minimized windows): resolve straight from `info` and, on success,
    /// remember it so a later restore can place/focus it.
    pub fn minimized_info(&mut self, info: &WindowInfo) -> Option<bool> {
        let el = self.resolve(info)?;
        let m = unsafe { read_minimized(el) };
        if m.is_some() {
            self.known.entry(info.id).or_insert_with(|| info.clone());
        }
        m
    }

    /// Minimize (`true`) or restore (`false`) the window via `AXMinimized`.
    pub fn set_minimized(&mut self, window: WindowId, minimized: bool) {
        let Some(info) = self.known.get(&window).cloned() else { return };
        let Some(el) = self.resolve(&info) else { return };
        unsafe {
            let attr = CFString::from_static_string("AXMinimized");
            let val = if minimized { CFBoolean::true_value() } else { CFBoolean::false_value() };
            AXUIElementSetAttributeValue(el, attr.as_concrete_TypeRef(), val.as_CFTypeRef());
        }
    }
}

impl Default for MacBackend {
    fn default() -> Self {
        Self::new()
    }
}

unsafe fn set_point(el: AXUIElementRef, x: f64, y: f64) {
    let point = CGPoint { x, y };
    let value = AXValueCreate(kAXValueTypeCGPoint, &point as *const _ as *const _);
    let attr = CFString::from_static_string(kAXPositionAttribute);
    AXUIElementSetAttributeValue(el, attr.as_concrete_TypeRef(), value as CFTypeRef);
    CFRelease(value as CFTypeRef);
}

unsafe fn read_minimized(el: AXUIElementRef) -> Option<bool> {
    let attr = CFString::from_static_string("AXMinimized");
    let mut value: CFTypeRef = std::ptr::null();
    let err = AXUIElementCopyAttributeValue(el, attr.as_concrete_TypeRef(), &mut value);
    if err != kAXErrorSuccess || value.is_null() {
        return None;
    }
    Some(CFBoolean::wrap_under_create_rule(value as _) == CFBoolean::true_value())
}

unsafe fn set_size(el: AXUIElementRef, w: f64, h: f64) {
    let size = CGSize { width: w, height: h };
    let value = AXValueCreate(kAXValueTypeCGSize, &size as *const _ as *const _);
    let attr = CFString::from_static_string(kAXSizeAttribute);
    AXUIElementSetAttributeValue(el, attr.as_concrete_TypeRef(), value as CFTypeRef);
    CFRelease(value as CFTypeRef);
}

impl Backend for MacBackend {
    fn screens(&self) -> Vec<Rect> {
        crate::cg::screens()
    }

    fn list_windows(&mut self) -> Vec<WindowInfo> {
        let windows = crate::cg::raw_windows();
        // Merge (not replace): a minimized window drops out of the on-screen list
        // but we must keep its info (pid) so it stays resolvable for restore.
        // `forget` prunes entries when a window is actually removed.
        for w in &windows {
            self.known.insert(w.id, w.clone());
        }
        windows
    }

    fn set_frame(&mut self, window: WindowId, frame: Rect) {
        let Some(info) = self.known.get(&window).cloned() else { return };
        let Some(el) = self.resolve(&info) else { return };
        unsafe {
            // Size first: setting the position while the window is still its old
            // (often full-screen) size lets macOS clamp the top-left against that
            // size, leaving a menu-bar-sized gap at the top. Shrink first, then
            // position, then re-assert size in case the move nudged it.
            set_size(el, frame.w, frame.h);
            set_point(el, frame.x, frame.y);
            set_size(el, frame.w, frame.h);
        }
    }

    fn focus(&mut self, window: WindowId) {
        let Some(info) = self.known.get(&window).cloned() else { return };
        let Some(el) = self.resolve(&info) else { return };
        unsafe {
            focus_window_skylight(info.pid, info.id.0 as u32, el);
        }
    }

    /// Raise a window above other tabs' windows (co-location restack on a tab
    /// switch) WITHOUT forcing keyboard focus to it — unlike `focus`, which uses
    /// SkyLight to actively front the window (and would flicker focus onto a
    /// co-placed window on another display).
    fn raise(&mut self, window: WindowId) {
        let Some(info) = self.known.get(&window).cloned() else { return };
        let Some(el) = self.resolve(&info) else { return };
        // AXRaise + app-frontmost: pure Accessibility has no focus-free way to
        // raise a window above OTHER apps' windows, so bringing every pane of a
        // split forward means fronting each pane's app. `focus` (SkyLight) runs
        // last and lands focus on the target, so any focus flick onto a sibling
        // is momentary — the cost of making the whole tab visible without a
        // window-server scripting addition.
        unsafe {
            let r = CFString::from_static_string(kAXRaiseAction);
            AXUIElementPerformAction(el, r.as_concrete_TypeRef());
            let app = AXUIElementCreateApplication(info.pid);
            if !app.is_null() {
                let attr = CFString::from_static_string(kAXFrontmostAttribute);
                let t = CFBoolean::true_value();
                AXUIElementSetAttributeValue(app, attr.as_concrete_TypeRef(), t.as_CFTypeRef());
                CFRelease(app as CFTypeRef);
            }
        }
    }

    fn forget(&mut self, window: WindowId) {
        // Dropping the AxElement releases the retained AXUIElement.
        self.handles.remove(&window);
        self.known.remove(&window);
    }
}
