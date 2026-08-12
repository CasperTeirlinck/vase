//! Thin conversions between Win32 handles/strings and the core's plain types.

use vase_core::geometry::Rect;
use vase_core::tree::WindowId;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::WindowsAndMessaging::{GetWindowLongPtrW, GWL_EXSTYLE, GWL_STYLE};

/// A window's id is its `HWND`: unique while the window lives, reused afterwards.
pub fn id_of(hwnd: HWND) -> WindowId {
    WindowId(hwnd.0 as usize as u64)
}

pub fn hwnd_of(id: WindowId) -> HWND {
    HWND(id.0 as usize as *mut core::ffi::c_void)
}

/// A `RECT` (left/top/right/bottom) as the core's origin+size rectangle.
pub fn rect_of(r: RECT) -> Rect {
    Rect::new(r.left as f64, r.top as f64, (r.right - r.left) as f64, (r.bottom - r.top) as f64)
}

pub fn style(hwnd: HWND) -> u32 {
    unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) as u32 }
}

pub fn ex_style(hwnd: HWND) -> u32 {
    unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32 }
}

/// Decode a NUL-terminated UTF-16 buffer that an API filled to `len` code units.
pub fn from_wide(buf: &[u16], len: usize) -> String {
    String::from_utf16_lossy(&buf[..len.min(buf.len())])
}

pub fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
