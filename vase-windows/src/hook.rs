//! Global low-level keyboard/mouse hooks → pure `KeyRouter` → consume/pass decisions.
//! Safety: default pass-through, hardcoded kill chord, panic containment, RAII teardown.

use std::cell::RefCell;
use std::io::Write;

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT, MSLLHOOKSTRUCT, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN, WM_LBUTTONDOWN, WM_SYSKEYDOWN,
};

use vase_core::input::{Decision, InputCommand, Key, KeyRouter, Mods};

use crate::keycode::{key_code, VK_ESCAPE};

/// Swallow the event.
const CONSUME: LRESULT = LRESULT(1);

/// Stamped on the masking keystroke below, so the hook can tell it apart from a real one.
const INJECTED_BY_VASE: usize = 0x7661_7365;

/// Tap Ctrl so the app sees a key between Alt going down and coming back up.
fn mask_modifier() {
    // A swallowed chord leaves the app having seen Alt pressed and released with nothing in between,
    // which is precisely the gesture that opens a menu bar. Ctrl masks it by doing nothing on its own.
    let tap = |flags: KEYBD_EVENT_FLAGS| INPUT { r#type: INPUT_KEYBOARD, Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: VK_CONTROL, wScan: 0, dwFlags: flags, time: 0, dwExtraInfo: INJECTED_BY_VASE } } };
    let taps = [tap(KEYBD_EVENT_FLAGS(0)), tap(KEYEVENTF_KEYUP)];
    unsafe { SendInput(&taps, std::mem::size_of::<INPUT>() as i32) };
}

struct State {
    router: KeyRouter,
    on_command: Box<dyn Fn(InputCommand)>,
    on_click: Box<dyn Fn((f64, f64)) -> bool>,
    on_key_intercept: Box<dyn Fn(Key) -> bool>,
    on_arm: Box<dyn Fn(bool)>,
}

// The hook callbacks are bare `extern "system"` functions with nowhere to carry a payload, so the router lives here.
thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

/// The installed hooks; dropping this unhooks them.
pub struct Hooks {
    keyboard: HHOOK,
    mouse: HHOOK,
}

impl Drop for Hooks {
    fn drop(&mut self) {
        let _ = unsafe { UnhookWindowsHookEx(self.keyboard) };
        let _ = unsafe { UnhookWindowsHookEx(self.mouse) };
        STATE.with(|s| *s.borrow_mut() = None);
    }
}

impl Hooks {
    /// Install the hooks on the current thread. The caller must pump a message loop on this same
    /// thread, or Windows never delivers to the callbacks and silently drops the hooks.
    pub fn install(
        router: KeyRouter,
        on_command: Box<dyn Fn(InputCommand)>,
        on_click: Box<dyn Fn((f64, f64)) -> bool>,
        on_key_intercept: Box<dyn Fn(Key) -> bool>,
        on_arm: Box<dyn Fn(bool)>,
    ) -> Option<Hooks> {
        STATE.with(|s| *s.borrow_mut() = Some(State { router, on_command, on_click, on_key_intercept, on_arm }));
        // A low-level hook needs no module handle when the callback lives in this process.
        let keyboard = unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), None, 0) }.ok()?;
        let mouse = unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), None, 0) }.ok()?;
        Some(Hooks { keyboard, mouse })
    }
}

fn held(vk: VIRTUAL_KEY) -> bool {
    // The high bit of GetAsyncKeyState is "currently down".
    (unsafe { GetAsyncKeyState(vk.0 as i32) } as u16 & 0x8000) != 0
}

fn mods() -> Mods {
    Mods { meta: held(VK_LWIN) || held(VK_RWIN), ctrl: held(VK_CONTROL), alt: held(VK_MENU), shift: held(VK_SHIFT) }
}

extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // Safety layer 1, extended: a panic in the logic below must leak the event through rather than
    // eat it or unwind across the FFI boundary.
    let decided = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| handle_key(code, wparam, lparam))).unwrap_or_else(|_| {
        let _ = writeln!(std::io::stderr(), "vase: keyboard hook panicked; passing event through");
        false
    });
    if decided {
        return CONSUME;
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

/// Returns whether the key was swallowed.
fn handle_key(code: i32, wparam: WPARAM, lparam: LPARAM) -> bool {
    if code < 0 {
        return false;
    }
    // Only key-down. Alt chords (the prefix is one) arrive as WM_SYSKEYDOWN, not WM_KEYDOWN.
    if wparam.0 as u32 != WM_KEYDOWN && wparam.0 as u32 != WM_SYSKEYDOWN {
        return false;
    }
    // SAFETY: for WH_KEYBOARD_LL with code >= 0, lparam is a KBDLLHOOKSTRUCT owned by the OS.
    let info = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
    if info.dwExtraInfo == INJECTED_BY_VASE {
        return false;
    }
    let vk = info.vkCode;
    let mods = mods();

    // Safety layer 3: the kill chord releases the keyboard whatever the router is doing.
    if vk == VK_ESCAPE && mods.meta && mods.ctrl && mods.alt {
        crate::request_quit();
        return true;
    }

    // A key vase has no name for can match no binding, so it passes straight through.
    let Some(kc) = key_code(vk) else { return false };
    let key = Key { code: kc, mods };

    STATE.with(|cell| {
        // Modal overlays get first refusal, before the router. The shared borrow ends with the statement,
        // so it cannot overlap the `borrow_mut` below; the callback reaches into the daemon's cell, not this one.
        let intercepted = match cell.borrow().as_ref() {
            Some(st) => (st.on_key_intercept)(key),
            None => return false,
        };
        if intercepted {
            return true;
        }
        let mut st = cell.borrow_mut();
        let Some(st) = st.as_mut() else { return false };
        // Safety layer 1: default pass-through unless the router explicitly consumed.
        let was_armed = st.router.is_armed();
        let decision = st.router.key(key);
        let now_armed = st.router.is_armed();
        if now_armed != was_armed {
            (st.on_arm)(now_armed);
        }
        let consumed = match decision {
            Decision::PassThrough => false,
            Decision::Consume => true,
            Decision::ConsumeAndRun(cmd) => {
                (st.on_command)(cmd);
                true
            }
        };
        // Only the modifiers that make a window do something on their own need masking.
        if consumed && (key.mods.alt || key.mods.meta) {
            mask_modifier();
        }
        consumed
    })
}

extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let consumed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if code < 0 || wparam.0 as u32 != WM_LBUTTONDOWN {
            return false;
        }
        // SAFETY: for WH_MOUSE_LL with code >= 0, lparam is an MSLLHOOKSTRUCT owned by the OS.
        let info = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
        let point = (info.pt.x as f64, info.pt.y as f64);
        STATE.with(|cell| {
            let st = cell.borrow();
            st.as_ref().is_some_and(|st| (st.on_click)(point))
        })
    }))
    .unwrap_or_else(|_| {
        let _ = writeln!(std::io::stderr(), "vase: mouse hook panicked; passing event through");
        false
    });
    if consumed {
        return CONSUME;
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}
