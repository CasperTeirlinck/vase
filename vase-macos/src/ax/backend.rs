use std::collections::HashSet;

use accessibility_sys::{
    kAXErrorSuccess, kAXFrontmostAttribute, kAXPositionAttribute, kAXRaiseAction, kAXSizeAttribute, kAXValueTypeCGPoint, kAXValueTypeCGSize, AXUIElementCopyAttributeValue,
    AXUIElementCreateApplication, AXUIElementPerformAction, AXUIElementRef, AXUIElementSetAttributeValue, AXValueCreate,
};
use core_foundation::base::{CFRelease, CFTypeRef, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::string::CFString;
use core_graphics::geometry::{CGPoint, CGSize};
use vase_core::backend::{Backend, Display, WindowInfo};
use vase_core::geometry::Rect;
use vase_core::tree::WindowId;

use super::skylight::focus_window_skylight;
use super::{read_bool_attr, read_string_attr, MacBackend};

unsafe fn set_point(el: AXUIElementRef, x: f64, y: f64) {
    let point = CGPoint { x, y };
    let value = AXValueCreate(kAXValueTypeCGPoint, &point as *const _ as *const _);
    let attr = CFString::from_static_string(kAXPositionAttribute);
    AXUIElementSetAttributeValue(el, attr.as_concrete_TypeRef(), value as CFTypeRef);
    CFRelease(value as CFTypeRef);
}

unsafe fn set_size(el: AXUIElementRef, w: f64, h: f64) {
    let size = CGSize { width: w, height: h };
    let value = AXValueCreate(kAXValueTypeCGSize, &size as *const _ as *const _);
    let attr = CFString::from_static_string(kAXSizeAttribute);
    AXUIElementSetAttributeValue(el, attr.as_concrete_TypeRef(), value as CFTypeRef);
    CFRelease(value as CFTypeRef);
}

impl Backend for MacBackend {
    fn displays(&self) -> Vec<Display> {
        crate::overlay::all_screens(self.mtm)
    }

    fn list_windows(&mut self) -> Vec<WindowInfo> {
        let windows = crate::cg::raw_windows();
        // Merge, don't replace: a minimized window drops off the on-screen list, but its info (pid) must stay resolvable for restore; `forget` prunes on real removal.
        for w in &windows {
            self.known.insert(w.id, w.clone());
        }
        windows
    }

    fn all_windows(&mut self) -> Vec<WindowInfo> {
        crate::cg::all_windows()
    }

    fn set_frame(&mut self, window: WindowId, frame: Rect) {
        let Some(el) = self.resolve_known(window) else { return };
        unsafe {
            // Size first: positioning at the old (often full-screen) size lets macOS clamp the top-left, leaving a menu-bar gap. Shrink, position, then re-assert size in case the move nudged it.
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

    /// Raise a window above other tabs' windows without giving it keyboard focus.
    fn raise(&mut self, window: WindowId) {
        let Some(info) = self.known.get(&window).cloned() else { return };
        let Some(el) = self.resolve(&info) else { return };
        // AXRaise + app-frontmost: pure Accessibility can't raise a window above OTHER apps' windows without fronting its app, so each pane's app is fronted. `focus` (SkyLight) runs last
        // and lands focus on the target, so any focus flick onto a sibling is momentary.
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

    /// `AXTitle`, unlike `kCGWindowName`, needs no Screen Recording grant and never goes stale.
    fn title(&mut self, window: WindowId) -> Option<String> {
        let el = self.resolve_known(window)?;
        unsafe { read_string_attr(el, "AXTitle") }
    }

    /// A transient popup (download bubble, panel) reports a non-standard subrole; adopting one flickers.
    fn tileable(&mut self, info: &WindowInfo) -> Option<bool> {
        let el = self.resolve(info)?;
        let subrole = unsafe { read_string_attr(el, "AXSubrole") }?;
        Some(subrole == "AXStandardWindow")
    }

    fn minimized(&mut self, window: WindowId) -> Option<bool> {
        let el = self.resolve_known(window)?;
        unsafe { read_bool_attr(el, "AXMinimized") }
    }

    fn minimized_info(&mut self, info: &WindowInfo) -> Option<bool> {
        let el = self.resolve(info)?;
        let m = unsafe { read_bool_attr(el, "AXMinimized") };
        if m.is_some() {
            self.known.entry(info.id).or_insert_with(|| info.clone());
        }
        m
    }

    fn set_minimized(&mut self, window: WindowId, minimized: bool) {
        let Some(el) = self.resolve_known(window) else { return };
        unsafe {
            let attr = CFString::from_static_string("AXMinimized");
            let val = if minimized { CFBoolean::true_value() } else { CFBoolean::false_value() };
            AXUIElementSetAttributeValue(el, attr.as_concrete_TypeRef(), val.as_CFTypeRef());
        }
    }

    /// Native macOS fullscreen: the window owns its own Space.
    fn fullscreen(&mut self, info: &WindowInfo) -> Option<bool> {
        let el = self.resolve(info)?;
        unsafe { read_bool_attr(el, "AXFullScreen") }
    }

    fn close(&mut self, window: WindowId) {
        let Some(el) = self.resolve_known(window) else { return };
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

    /// `.app` file stems from the standard macOS app directories.
    fn launchable_apps(&self) -> Vec<String> {
        let home = std::env::var("HOME").unwrap_or_default();
        let dirs = ["/Applications".to_string(), "/System/Applications".to_string(), "/System/Applications/Utilities".to_string(), format!("{home}/Applications")];
        let mut apps: Vec<String> = Vec::new();
        for dir in &dirs {
            let Ok(entries) = std::fs::read_dir(dir) else { continue };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("app") {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        apps.push(stem.to_string());
                    }
                }
            }
        }
        // Finder lives in /System/Library/CoreServices (not scanned), but it's a normal launchable app.
        apps.push("Finder".to_string());
        apps.sort_by_key(|a| a.to_lowercase());
        apps.dedup();
        apps
    }

    fn launch(&self, app: &str) {
        // `-n` opens a fresh instance so an already-running app still yields a new window for the pane. Singletons
        // refuse `-n`, so fall back to plain activation. Finder won't open a window on activation, so point it at $HOME.
        let cmd = if app == "Finder" {
            "open ~".to_string()
        } else {
            let q = app.replace('\'', r"'\''");
            format!("open -na '{q}' || open -a '{q}'")
        };
        if let Err(e) = std::process::Command::new("sh").arg("-c").arg(&cmd).spawn() {
            eprintln!("failed to launch {app}: {e}");
        }
    }

    fn badged_apps(&self) -> HashSet<String> {
        crate::dock::badged_apps()
    }
}
