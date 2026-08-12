use std::collections::HashMap;

use accessibility_sys::{kAXErrorSuccess, kAXWindowsAttribute, AXUIElementCopyAttributeValue, AXUIElementCreateApplication, AXUIElementRef};
use core_foundation::array::CFArray;
use core_foundation::base::{CFRelease, CFRetain, CFType, CFTypeRef, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::string::CFString;
use objc2::MainThreadMarker;
use vase_core::backend::WindowInfo;
use vase_core::tree::WindowId;

mod backend;
mod skylight;

use skylight::_AXUIElementGetWindow;

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
    /// Displays are read through NSScreen, which is main-thread only.
    mtm: MainThreadMarker,
}

impl MacBackend {
    pub fn new(mtm: MainThreadMarker) -> Self {
        MacBackend { handles: HashMap::new(), known: HashMap::new(), mtm }
    }

    /// Find and retain the AXUIElement for `info`, matched by CoreGraphics window id.
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
                if _AXUIElementGetWindow(el, &mut cg_id) == kAXErrorSuccess && cg_id as u64 == info.id.0 {
                    let retained = CFRetain(el as CFTypeRef) as AXUIElementRef;
                    self.handles.insert(info.id, AxElement(retained));
                    return Some(retained);
                }
            }
        }
        None
    }

    /// Resolve an element for a window we already track.
    fn resolve_known(&mut self, window: WindowId) -> Option<AXUIElementRef> {
        let info = self.known.get(&window).cloned()?;
        self.resolve(&info)
    }
}

unsafe fn read_bool_attr(el: AXUIElementRef, name: &'static str) -> Option<bool> {
    let attr = CFString::from_static_string(name);
    let mut value: CFTypeRef = std::ptr::null();
    let err = AXUIElementCopyAttributeValue(el, attr.as_concrete_TypeRef(), &mut value);
    if err != kAXErrorSuccess || value.is_null() {
        return None;
    }
    Some(CFBoolean::wrap_under_create_rule(value as _) == CFBoolean::true_value())
}

unsafe fn read_string_attr(el: AXUIElementRef, name: &'static str) -> Option<String> {
    let attr = CFString::from_static_string(name);
    let mut value: CFTypeRef = std::ptr::null();
    let err = AXUIElementCopyAttributeValue(el, attr.as_concrete_TypeRef(), &mut value);
    if err != kAXErrorSuccess || value.is_null() {
        return None;
    }
    Some(CFString::wrap_under_create_rule(value as _).to_string())
}
