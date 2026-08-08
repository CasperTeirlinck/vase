//! Global CGEventTap → pure KeyRouter → consume/pass decisions.
//! Safety: default pass-through, hardcoded kill chord, panic containment, RAII teardown, best-effort re-enable on OS-initiated disable.

use std::cell::{Cell, RefCell};
use std::io::Write;
use std::rc::Rc;

use core_foundation::runloop::{kCFRunLoopCommonModes, CFRunLoop};
use core_graphics::event::{CGEvent, CGEventFlags, CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement, CGEventType, CallbackResult, EventField};
use vase_core::input::keys::VK_ESC;
use vase_core::input::{Decision, InputCommand, Key, KeyRouter, Mods};

struct State {
    router: KeyRouter,
    on_command: Box<dyn Fn(InputCommand)>,
    on_click: Box<dyn Fn((f64, f64)) -> bool>,
    on_key_intercept: Box<dyn Fn(Key) -> bool>,
    on_arm: Box<dyn Fn(bool)>,
}

pub struct EventTap {
    tap: CGEventTap<'static>,
    needs_reenable: Rc<Cell<bool>>,
}

impl EventTap {
    /// Install the tap on the current thread's run loop; `None` if it can't be created (missing Accessibility/Input-Monitoring permission).
    /// The caller must drive the CFRunLoop from this same thread (the callback is thread-confined).
    pub fn install(
        router: KeyRouter,
        on_command: Box<dyn Fn(InputCommand)>,
        on_click: Box<dyn Fn((f64, f64)) -> bool>,
        on_key_intercept: Box<dyn Fn(Key) -> bool>,
        on_arm: Box<dyn Fn(bool)>,
    ) -> Option<EventTap> {
        let state = RefCell::new(State { router, on_command, on_click, on_key_intercept, on_arm });
        let needs_reenable = Rc::new(Cell::new(false));
        let needs_reenable_cb = Rc::clone(&needs_reenable);

        // Only KeyDown is needed: the prefix chord arrives as a KeyDown.
        // Deliberately not tapping FlagsChanged lets modifier press/release flow to apps untouched, so consuming the prefix keydown never leaves a modifier stuck.
        // TapDisabled* sentinels arrive regardless of the mask (and must not be listed, since their values overflow it).
        let events = vec![CGEventType::KeyDown, CGEventType::LeftMouseDown];

        // SAFETY: `new_unchecked` requires that state captured by the callback be safe to send across threads, OR that the tap only ever be enabled on the current thread's run loop.
        // We choose the latter: `state` and `needs_reenable_cb` are `Rc`/`RefCell`-based (thread-confined) and `install` documents that the caller must drive the run loop from this same thread,
        // so the callback is never invoked from elsewhere.
        let tap = unsafe {
            CGEventTap::new_unchecked(CGEventTapLocation::Session, CGEventTapPlacement::HeadInsertEventTap, CGEventTapOptions::Default, events, move |_proxy, event_type, event| {
                callback(&state, &needs_reenable_cb, event_type, event)
            })
        }
        .ok()?;

        let source = tap.mach_port().create_runloop_source(0).ok()?;
        CFRunLoop::get_current().add_source(&source, unsafe { kCFRunLoopCommonModes });
        tap.enable();
        Some(EventTap { tap, needs_reenable })
    }

    /// Safety layer 4 (best-effort): if macOS disabled the tap (timeout or user-toggle), re-enable it. Call periodically from the run-loop driver.
    pub fn poll_reenable(&self) {
        if self.needs_reenable.get() {
            self.needs_reenable.set(false);
            self.tap.enable();
        }
    }
}

// Teardown (safety layer 4) is inherited from `CGEventTap`'s own `Drop`, which invalidates the tap's mach port; dropping `EventTap` unhooks it.

fn callback(state: &RefCell<State>, needs_reenable: &Rc<Cell<bool>>, event_type: CGEventType, event: &CGEvent) -> CallbackResult {
    // Safety layer 1, extended: a panic in the logic below must still leak the event through rather than eat it or abort the whole tap.
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| handle(state, needs_reenable, event_type, event))).unwrap_or_else(|_| {
        let _ = writeln!(std::io::stderr(), "vase: event-tap callback panicked; passing event through");
        CallbackResult::Keep
    })
}

fn handle(state: &RefCell<State>, needs_reenable: &Rc<Cell<bool>>, event_type: CGEventType, event: &CGEvent) -> CallbackResult {
    if matches!(event_type, CGEventType::TapDisabledByTimeout | CGEventType::TapDisabledByUserInput) {
        let _ = writeln!(std::io::stderr(), "vase: event tap disabled by macOS ({event_type:?}); flagging for re-enable");
        needs_reenable.set(true);
        return CallbackResult::Keep;
    }

    if matches!(event_type, CGEventType::LeftMouseDown) {
        let loc = event.location(); // global, top-left coords
        let handled = {
            let st = state.borrow();
            (st.on_click)((loc.x, loc.y))
        };
        return if handled { CallbackResult::Drop } else { CallbackResult::Keep };
    }

    let code = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE);
    let flags = event.get_flags();
    let cmd = flags.contains(CGEventFlags::CGEventFlagCommand);
    let ctrl = flags.contains(CGEventFlags::CGEventFlagControl);
    let alt = flags.contains(CGEventFlags::CGEventFlagAlternate);
    let shift = flags.contains(CGEventFlags::CGEventFlagShift);

    // Safety layer 3: hardcoded kill chord Ctrl+Alt+Cmd+Esc → release keyboard.
    if code as u16 == VK_ESC && cmd && ctrl && alt {
        crate::request_quit();
        return CallbackResult::Drop;
    }

    // Only KeyDown is tapped; anything else passes straight through.
    if !matches!(event_type, CGEventType::KeyDown) {
        return CallbackResult::Keep;
    }

    let key = Key { code: code as u16, mods: Mods { cmd, ctrl, alt, shift } };

    // Modal overlays (e.g. the switcher) get first dibs on the key, before the router. Borrow released here
    // so it doesn't overlap the `borrow_mut` below (the intercept closure reaches into a separate RefCell).
    if (state.borrow().on_key_intercept)(key) {
        return CallbackResult::Drop;
    }

    let mut st = state.borrow_mut();
    // Safety layer 1: default pass-through unless the router explicitly consumed.
    let was_armed = st.router.is_armed();
    let decision = st.router.key(key);
    let now_armed = st.router.is_armed();
    if now_armed != was_armed {
        (st.on_arm)(now_armed);
    }
    match decision {
        Decision::PassThrough => CallbackResult::Keep,
        Decision::Consume => CallbackResult::Drop,
        Decision::ConsumeAndRun(cmd) => {
            (st.on_command)(cmd);
            CallbackResult::Drop
        }
    }
}
