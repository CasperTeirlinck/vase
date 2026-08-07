//! CoreGraphics: display geometry and on-screen window enumeration. Read-only.

use core_foundation::base::{CFType, TCFType};
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use core_graphics::display::CGDisplay;
use core_graphics::window::{
    copy_window_info, kCGNullWindowID, kCGWindowListExcludeDesktopElements,
    kCGWindowListOptionOnScreenOnly,
};
use vase_core::backend::WindowInfo;
use vase_core::geometry::Rect;
use vase_core::tree::WindowId;

/// Visible display rects in global coordinates.
pub fn screens() -> Vec<Rect> {
    CGDisplay::active_displays()
        .unwrap_or_default()
        .into_iter()
        .map(|id| {
            let b = CGDisplay::new(id).bounds();
            Rect::new(b.origin.x, b.origin.y, b.size.width, b.size.height)
        })
        .collect()
}

fn dict_i64(dict: &CFDictionary, key: &str) -> Option<i64> {
    let k = CFString::new(key);
    let v = dict.find(k.as_CFTypeRef())?;
    let num = unsafe { CFType::wrap_under_get_rule(*v as _) };
    num.downcast::<CFNumber>()?.to_i64()
}

fn dict_f64(dict: &CFDictionary, key: &str) -> Option<f64> {
    let k = CFString::new(key);
    let v = dict.find(k.as_CFTypeRef())?;
    let num = unsafe { CFType::wrap_under_get_rule(*v as _) };
    num.downcast::<CFNumber>()?.to_f64()
}

fn dict_string(dict: &CFDictionary, key: &str) -> Option<String> {
    let k = CFString::new(key);
    let v = dict.find(k.as_CFTypeRef())?;
    let s = unsafe { CFType::wrap_under_get_rule(*v as _) };
    Some(s.downcast::<CFString>()?.to_string())
}

fn dict_rect(dict: &CFDictionary, key: &str) -> Option<Rect> {
    let k = CFString::new(key);
    let v = dict.find(k.as_CFTypeRef())?;
    let bounds = unsafe { CFType::wrap_under_get_rule(*v as _) }.downcast::<CFDictionary>()?;
    Some(Rect::new(
        dict_f64(&bounds, "X")?,
        dict_f64(&bounds, "Y")?,
        dict_f64(&bounds, "Width")?,
        dict_f64(&bounds, "Height")?,
    ))
}

/// Every on-screen window as a `WindowInfo` (id = CoreGraphics window number).
pub fn raw_windows() -> Vec<WindowInfo> {
    windows_with(kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements)
}

/// Every window (all Spaces, including minimized/off-screen ones), minus desktop
/// elements. Broader than `raw_windows`; callers must filter (e.g. by AX
/// `AXMinimized`) since this also includes other Spaces' and background windows.
pub fn all_windows() -> Vec<WindowInfo> {
    windows_with(kCGWindowListExcludeDesktopElements)
}

fn windows_with(options: u32) -> Vec<WindowInfo> {
    let Some(list) = copy_window_info(options, kCGNullWindowID) else {
        return Vec::new();
    };
    // Each element is a CFDictionaryRef describing one window; the array itself
    // is untyped (`*const c_void`), so downcast every entry through CFType.
    // https://developer.apple.com/documentation/coregraphics/1454852-cgwindowlistcopywindowinfo
    list.iter()
        .filter_map(|raw| {
            let dict =
                unsafe { CFType::wrap_under_get_rule(*raw as _) }.downcast::<CFDictionary>()?;
            Some(WindowInfo {
                id: WindowId(dict_i64(&dict, "kCGWindowNumber")? as u64),
                pid: dict_i64(&dict, "kCGWindowOwnerPID")? as i32,
                app: dict_string(&dict, "kCGWindowOwnerName").unwrap_or_default(),
                title: dict_string(&dict, "kCGWindowName").unwrap_or_default(),
                frame: dict_rect(&dict, "kCGWindowBounds")?,
                layer: dict_i64(&dict, "kCGWindowLayer").unwrap_or(0),
            })
        })
        .collect()
}
