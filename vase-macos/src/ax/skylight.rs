use accessibility_sys::{kAXRaiseAction, AXUIElementPerformAction, AXUIElementRef};
use core_foundation::base::TCFType;
use core_foundation::string::CFString;

// Private HIServices function mapping an AXUIElement to its CoreGraphics window id. Not exposed by accessibility-sys; links via ApplicationServices.
extern "C" {
    pub(super) fn _AXUIElementGetWindow(element: AXUIElementRef, out: *mut u32) -> i32;
}

// Carbon Process Manager + private SkyLight functions to focus a *specific* window across displays/Spaces (pure Accessibility only focuses the app's window on the active display).
// GetProcessForPID links via ApplicationServices; the SLPS* symbols via SkyLight (see build.rs).
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

/// Give a specific window keyboard focus regardless of display or Space.
pub(super) unsafe fn focus_window_skylight(pid: i32, window_id: u32, el: AXUIElementRef) {
    let mut psn = ProcessSerialNumber { high: 0, low: 0 };
    if GetProcessForPID(pid, &mut psn) != 0 {
        return;
    }
    _SLPSSetFrontProcessWithOptions(&psn, window_id, K_CPS_USER_GENERATED);
    // Synthetic "make key" event records (byte layout from yabai): window id at offset 0x3c; the second record differs only in the 0x08 tag (0x01, 0x02).
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
