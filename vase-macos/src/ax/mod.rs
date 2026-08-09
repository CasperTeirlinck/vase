use std::collections::HashMap;

use accessibility_sys::{kAXErrorSuccess, kAXWindowsAttribute, AXUIElementCopyAttributeValue, AXUIElementCreateApplication, AXUIElementPerformAction, AXUIElementRef, AXUIElementSetAttributeValue};
use core_foundation::array::CFArray;
use core_foundation::base::{CFRelease, CFRetain, CFType, CFTypeRef, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::string::CFString;
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
}

impl MacBackend {
    pub fn new() -> Self {
        MacBackend { handles: HashMap::new(), known: HashMap::new() }
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

    /// The window's live title via Accessibility `AXTitle` (unlike `kCGWindowName`, which needs Screen Recording and goes stale).
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

    /// The window's Accessibility subrole (e.g. `AXStandardWindow`, `AXDialog`), used to skip transient popups that shouldn't be tiled.
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

    /// Close a window by pressing its AX close button.
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

    /// Whether the window is minimized to the Dock; `None` if it can't be read.
    pub fn minimized(&mut self, window: WindowId) -> Option<bool> {
        let info = self.known.get(&window).cloned()?;
        let el = self.resolve(&info)?;
        unsafe { read_bool_attr(el, "AXMinimized") }
    }

    /// Like `minimized`, but for an untracked window; remembers it on success.
    pub fn minimized_info(&mut self, info: &WindowInfo) -> Option<bool> {
        let el = self.resolve(info)?;
        let m = unsafe { read_bool_attr(el, "AXMinimized") };
        if m.is_some() {
            self.known.entry(info.id).or_insert_with(|| info.clone());
        }
        m
    }

    /// Whether the window is in native macOS fullscreen (its own Space); `None` if it can't be read.
    pub fn fullscreen(&mut self, info: &WindowInfo) -> Option<bool> {
        let el = self.resolve(info)?;
        unsafe { read_bool_attr(el, "AXFullScreen") }
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

unsafe fn read_bool_attr(el: AXUIElementRef, name: &'static str) -> Option<bool> {
    let attr = CFString::from_static_string(name);
    let mut value: CFTypeRef = std::ptr::null();
    let err = AXUIElementCopyAttributeValue(el, attr.as_concrete_TypeRef(), &mut value);
    if err != kAXErrorSuccess || value.is_null() {
        return None;
    }
    Some(CFBoolean::wrap_under_create_rule(value as _) == CFBoolean::true_value())
}
