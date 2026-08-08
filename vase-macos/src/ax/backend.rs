use accessibility_sys::{
    kAXFrontmostAttribute, kAXPositionAttribute, kAXRaiseAction, kAXSizeAttribute, kAXValueTypeCGPoint, kAXValueTypeCGSize, AXUIElementCreateApplication, AXUIElementPerformAction, AXUIElementRef,
    AXUIElementSetAttributeValue, AXValueCreate,
};
use core_foundation::base::{CFRelease, CFTypeRef, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::string::CFString;
use core_graphics::geometry::{CGPoint, CGSize};
use vase_core::backend::{Backend, WindowInfo};
use vase_core::geometry::Rect;
use vase_core::tree::WindowId;

use super::skylight::focus_window_skylight;
use super::MacBackend;

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
    fn screens(&self) -> Vec<Rect> {
        crate::cg::screens()
    }

    fn list_windows(&mut self) -> Vec<WindowInfo> {
        let windows = crate::cg::raw_windows();
        // Merge, don't replace: a minimized window drops off the on-screen list, but its info (pid) must stay resolvable for restore; `forget` prunes on real removal.
        for w in &windows {
            self.known.insert(w.id, w.clone());
        }
        windows
    }

    fn set_frame(&mut self, window: WindowId, frame: Rect) {
        let Some(info) = self.known.get(&window).cloned() else { return };
        let Some(el) = self.resolve(&info) else { return };
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
}
