use std::cell::RefCell;
use std::rc::Rc;

use windows::Win32::UI::HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
use windows::Win32::UI::WindowsAndMessaging::{DispatchMessageW, MsgWaitForMultipleObjectsEx, PeekMessageW, TranslateMessage, MSG, MWMO_INPUTAVAILABLE, PM_REMOVE, QS_ALLINPUT};

use vase_core::backend::{manageable, Backend};
use vase_core::daemon::{Daemon, Paths};
use vase_core::geometry::{screen_of, Rect};
use vase_core::input::{InputCommand, Key};
use vase_core::model::Effect;
use vase_core::registry::Registry;
use vase_core::state::LiveWindow;
use vase_windows::{D2DPainter, Hooks, WindowsBackend};

/// The daemon, bound to this platform's backend and painter.
type Vase = Daemon<WindowsBackend, D2DPainter>;

struct RestoreGuard(Rc<RefCell<Vase>>);
impl Drop for RestoreGuard {
    fn drop(&mut self) {
        self.0.borrow_mut().restore();
    }
}

fn main() {
    // Per-monitor v2 first: without it Windows virtualizes coordinates on a scaled display and every
    // frame vase sets would land in the wrong place.
    let _ = unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };

    let mut backend = WindowsBackend::new();
    // Displays come back ordered left-to-right, then top-to-bottom, so a screen's index matches its
    // physical layout (the tab bar groups tabs by screen in index order, and cycling traverses it the same way).
    let displays = backend.displays();
    if displays.is_empty() {
        eprintln!("vase: no displays found.");
        std::process::exit(1);
    }
    let display_ids: Vec<u32> = displays.iter().map(|d| d.id).collect();
    let screens_cg: Vec<Rect> = displays.iter().map(|d| d.bounds).collect();
    // The main display is the one at the virtual-desktop origin.
    let main_screen = screens_cg.iter().position(|r| r.x == 0.0 && r.y == 0.0).unwrap_or(0);
    let screen_rects: Vec<Rect> = displays.iter().enumerate().map(|(i, d)| vase_core::chrome::usable(d.work_area, i == main_screen)).collect();

    // Minimized windows stay in the enumeration on Windows, so one pass adopts everything; they show
    // as tabs but are not placed until selected.
    let current = backend.list_windows();
    let mut windows = Registry::default();
    let mut live = Vec::new();
    for w in current.iter().filter(|w| manageable(w)) {
        let minimized = backend.minimized(w.id) == Some(true);
        windows.adopt(w, minimized);
        live.push(LiveWindow { id: w.id, app: w.app.clone(), title: w.title.clone(), screen: screen_of(w.frame, &screens_cg) });
    }
    println!("adopted {} windows as tabs.", live.len());

    let paths = Paths { config: vase_windows::paths::config(), state: vase_windows::paths::state() };
    let saved = paths.state.as_deref().and_then(vase_core::state::load);
    if saved.is_some() {
        println!("restored saved layout.");
    }
    let model = vase_core::state::restore(saved, &live, &screen_rects);

    let painter = match D2DPainter::new() {
        Ok(painter) => painter,
        Err(e) => {
            eprintln!("vase: could not create the Direct2D chrome: {e}");
            std::process::exit(1);
        }
    };
    let daemon = Rc::new(RefCell::new(Vase::new(model, backend, painter, windows, paths, main_screen, screens_cg, display_ids)));
    // Armed before the hooks exist: nothing has been moved yet (adoption above only reads frames), so
    // a panic before install has nothing to undo; the guard just becomes a no-op restore.
    let _guard = RestoreGuard(Rc::clone(&daemon));

    let daemon_cb = Rc::clone(&daemon);
    let on_command = Box::new(move |cmd: InputCommand| daemon_cb.borrow_mut().run(cmd));

    let daemon_click = Rc::clone(&daemon);
    let on_click = Box::new(move |(px, py): (f64, f64)| -> bool { daemon_click.borrow_mut().click(px, py) });

    let daemon_key = Rc::clone(&daemon);
    let on_key_intercept = Box::new(move |key: Key| -> bool { daemon_key.borrow_mut().intercept_key(key) });

    let daemon_arm = Rc::clone(&daemon);
    let on_arm = Box::new(move |armed: bool| {
        let mut d = daemon_arm.borrow_mut();
        d.prefix_armed = armed;
        d.refresh(); // recolor the prefix dot
    });

    let router = vase_core::input::router();
    let _hooks = match Hooks::install(router, on_command, on_click, on_key_intercept, on_arm) {
        Some(hooks) => hooks,
        None => {
            eprintln!("vase: could not install the input hooks.");
            // Nothing has been moved yet (no render has run), so it is safe that process::exit skips the RestoreGuard destructor here.
            std::process::exit(1);
        }
    };

    // Only move windows once the hooks are confirmed live, so every reachable exit path from here on is
    // covered by the run loop + restore below.
    {
        let mut d = daemon.borrow_mut();
        let placements = d.model.as_ref().unwrap().placements();
        let mut effects = vec![Effect::Render(placements)];
        if let Some(w) = d.model.as_ref().unwrap().focused_window() {
            effects.push(Effect::FocusWindow(w));
        }
        d.execute(effects);
    }
    daemon.borrow_mut().warm_window_icons();
    daemon.borrow_mut().refresh();

    run_message_loop(&daemon);
    daemon.borrow_mut().restore();
    println!("vase daemon: stopped, windows restored.");
}

/// The message loop, ticking the daemon each wake. The low-level hooks are delivered to this thread,
/// so it has to keep pumping or Windows silently drops them.
fn run_message_loop(daemon: &Rc<RefCell<Vase>>) {
    let mut wake = 0u32;
    while !vase_windows::should_quit() && !daemon.borrow().quit_requested() {
        unsafe {
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            // Block up to ~25 ms for the next input (keeps the prefix-number timeout crisp).
            MsgWaitForMultipleObjectsEx(None, 25, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
        }
        daemon.borrow_mut().tick_tab_entry();
        daemon.borrow_mut().tick_switcher();
        daemon.borrow_mut().tick_pane_picker();
        daemon.borrow_mut().tick_reframe();
        wake += 1;
        // The window-list diff is expensive, so reconcile only every ~4th wake (~100 ms).
        if wake.is_multiple_of(4) {
            daemon.borrow_mut().reconcile();
            daemon.borrow_mut().warm_icons();
        }
    }
}
