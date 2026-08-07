//! Read which apps currently show a Dock notification badge, by scraping the
//! Dock's Accessibility tree (each dock item exposes `AXStatusLabel` — the red
//! badge's text — which is empty when the app has no badge). There is no public
//! API to read another app's Dock badge, so this is the pragmatic route.

use std::collections::HashSet;

use accessibility_sys::{
    kAXErrorSuccess, AXUIElementCopyAttributeValue, AXUIElementCreateApplication, AXUIElementRef,
};
use core_foundation::array::CFArray;
use core_foundation::base::{CFRelease, CFType, CFTypeRef, TCFType};
use core_foundation::string::CFString;
use objc2_app_kit::NSWorkspace;

/// Display names of apps whose Dock icon currently carries a badge.
pub fn badged_apps() -> HashSet<String> {
    let mut out = HashSet::new();
    let Some(pid) = dock_pid() else { return out };
    unsafe {
        let app = AXUIElementCreateApplication(pid);
        if app.is_null() {
            return out;
        }
        // Dock → its list(s) → the application dock items.
        if let Some(lists) = ax_children(app) {
            for list in lists.iter() {
                let list_el = list.as_CFTypeRef() as AXUIElementRef;
                let Some(items) = ax_children(list_el) else { continue };
                for item in items.iter() {
                    let item_el = item.as_CFTypeRef() as AXUIElementRef;
                    let badged = ax_string(item_el, "AXStatusLabel")
                        .is_some_and(|s| !s.trim().is_empty());
                    if badged {
                        if let Some(title) = ax_string(item_el, "AXTitle") {
                            out.insert(title);
                        }
                    }
                }
            }
        }
        CFRelease(app as CFTypeRef);
    }
    out
}

fn dock_pid() -> Option<i32> {
    let apps = NSWorkspace::sharedWorkspace().runningApplications();
    apps.iter().find_map(|a| {
        (a.bundleIdentifier()?.to_string() == "com.apple.dock").then(|| a.processIdentifier())
    })
}

/// The `AXChildren` of `el` as a CFArray (kept alive by the caller so its
/// elements stay valid while iterated).
unsafe fn ax_children(el: AXUIElementRef) -> Option<CFArray<CFType>> {
    let attr = CFString::from_static_string("AXChildren");
    let mut value: CFTypeRef = std::ptr::null();
    if AXUIElementCopyAttributeValue(el, attr.as_concrete_TypeRef(), &mut value) != kAXErrorSuccess
        || value.is_null()
    {
        return None;
    }
    Some(CFArray::wrap_under_create_rule(value as _))
}

unsafe fn ax_string(el: AXUIElementRef, attr: &str) -> Option<String> {
    let attr = CFString::new(attr);
    let mut value: CFTypeRef = std::ptr::null();
    if AXUIElementCopyAttributeValue(el, attr.as_concrete_TypeRef(), &mut value) != kAXErrorSuccess
        || value.is_null()
    {
        return None;
    }
    Some(CFString::wrap_under_create_rule(value as _).to_string())
}
