use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use objc2_app_kit::{NSApplication, NSEventMask};
use objc2_foundation::{NSDate, NSDefaultRunLoopMode};
use vase_core::backend::{manageable, Backend};
use vase_core::geometry::Rect;
use vase_core::input::{InputCommand, Key};
use vase_core::model::{Command, Effect};
use vase_core::tree::WindowId;
use vase_macos::daemon::{screen_of, Daemon};
use vase_macos::registry::Registry;
use vase_macos::state::LiveWindow;
use vase_macos::{nsapp_init, EventTap, MacBackend};

struct RestoreGuard(Rc<RefCell<Daemon>>);
impl Drop for RestoreGuard {
    fn drop(&mut self) {
        self.0.borrow_mut().restore();
    }
}

fn main() {
    let mtm = objc2::MainThreadMarker::new().unwrap();
    nsapp_init(mtm);
    let _status_bar = vase_macos::status::install(mtm); // Held until main returns, keeping the menu-bar item alive.

    let mut backend = MacBackend::new();
    let mut screens = vase_macos::overlay::all_screens(mtm);
    if screens.is_empty() {
        eprintln!("no screens - grant Accessibility (and Input Monitoring), then re-run.");
        std::process::exit(1);
    }
    sort_displays_by_position(&mut screens);
    // Stable display id + full CG (CoreGraphics) bounds per display. Ids match tabs to monitors across hotplug; full bounds assign a window to a monitor by center.
    let display_ids: Vec<u32> = screens.iter().map(|(id, _, _)| *id).collect();
    let screens_cg: Vec<Rect> = screens.iter().map(|(_, full, _)| *full).collect();
    // The main display is the one at the CG global origin (0,0).
    let main_screen = screens_cg.iter().position(|r| r.x == 0.0 && r.y == 0.0).unwrap_or(0);

    // Usable rect per monitor = that display's visible frame (already excludes its own menu bar and Dock); the main display also reserves a bottom strip for the tab bar.
    let screen_rects: Vec<Rect> =
        screens.iter().enumerate().map(|(i, (_, _, vis))| if i == main_screen { Rect::new(vis.x, vis.y, vis.w, vis.h - vase_macos::overlay::BAR_HEIGHT) } else { *vis }).collect();

    // Adopt every on-screen manageable window, plus the ones already minimized: scan the full window list (all Spaces) and keep only the ones AX confirms are minimized,
    // so we don't pull in other Spaces' or background windows. Minimized ones show as tabs but aren't placed until selected.
    let onscreen = backend.list_windows();
    let onscreen_ids: HashSet<WindowId> = onscreen.iter().map(|w| w.id).collect();
    let offscreen: Vec<_> = vase_macos::cg::all_windows().into_iter().filter(|w| !onscreen_ids.contains(&w.id) && manageable(w) && backend.minimized_info(w) == Some(true)).collect();
    let minimized: HashSet<WindowId> = offscreen.iter().map(|w| w.id).collect();

    let mut windows = Registry::default();
    let mut live = Vec::new();
    for w in onscreen.iter().filter(|w| manageable(w)).chain(offscreen.iter()) {
        windows.adopt(w, minimized.contains(&w.id));
        live.push(LiveWindow { id: w.id, app: w.app.clone(), title: w.title.clone(), screen: screen_of(w.frame, &screens_cg) });
    }
    if live.iter().any(|w| w.title.is_empty()) {
        eprintln!("warning: some window titles are empty; grant Screen Recording permission so titles show.");
    }
    println!("adopted {} windows as tabs.", live.len());

    let saved = vase_macos::state::load();
    if saved.is_some() {
        println!("restored saved layout.");
    }
    let model = vase_macos::state::restore(saved, &live, &screen_rects);

    let daemon = Rc::new(RefCell::new(Daemon::new(mtm, model, backend, windows, main_screen, screens_cg, display_ids)));
    // Armed now, before the tap exists: nothing has been moved yet (adoption above only reads frames), so a panic before install has nothing to undo; the guard just becomes a no-op restore.
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

    let router = vase_macos::keymap::router();
    let tap = match EventTap::install(router, on_command, on_click, on_key_intercept, on_arm) {
        Some(t) => t,
        None => {
            eprintln!("could not install event tap - grant Accessibility AND Input Monitoring.");
            // Nothing has been moved yet (no render has run), so it is safe that process::exit skips the RestoreGuard destructor here.
            std::process::exit(1);
        }
    };

    // Only move windows once the tap is confirmed live, so every reachable exit path from here on is covered by the run loop + restore below. Co-locate every adopted window at the screen rect once,
    // then raise the first tab on top. From here, cycling tabs is a pure raise (emits only FocusWindow); windows never move again until quit/restore.
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

    run_event_loop(mtm, &daemon, &tap);
    daemon.borrow_mut().restore();
    println!("vase daemon: stopped, windows restored.");
}

/// AppKit's event loop, ticking the daemon each wake, until the user quits, the kill chord, or a crash; nothing is parked off-screen, so an abrupt exit leaves windows visible.
fn run_event_loop(mtm: objc2::MainThreadMarker, daemon: &Rc<RefCell<Daemon>>, tap: &EventTap) {
    let app = NSApplication::sharedApplication(mtm);
    let mode = unsafe { NSDefaultRunLoopMode };
    let mut wake = 0u32;
    while !vase_macos::should_quit() {
        // Block up to ~25 ms for the next event (keeps the prefix-number timeout crisp), dispatch it, then drain any others.
        // Servicing events here also drives the CFRunLoop (the tap is on common modes) and lets the status-item menu act.
        let deadline = NSDate::dateWithTimeIntervalSinceNow(0.025);
        if let Some(event) = app.nextEventMatchingMask_untilDate_inMode_dequeue(NSEventMask::Any, Some(&deadline), mode, true) {
            app.sendEvent(&event);
            let past = NSDate::distantPast();
            while let Some(event) = app.nextEventMatchingMask_untilDate_inMode_dequeue(NSEventMask::Any, Some(&past), mode, true) {
                app.sendEvent(&event);
            }
        }
        tap.poll_reenable();
        // Menu-bar actions queued from the AppKit side.
        if vase_macos::take_new_tab() {
            daemon.borrow_mut().dispatch(Command::NewTab);
        }
        if vase_macos::take_reload_config() {
            daemon.borrow_mut().reload_config();
        }
        daemon.borrow_mut().tick_tab_entry();
        daemon.borrow_mut().tick_switcher();
        daemon.borrow_mut().tick_pane_picker();
        daemon.borrow_mut().tick_reframe();
        wake += 1;
        // The window-list diff is expensive, so reconcile only every ~4th wake (~100 ms).
        if wake % 4 == 0 {
            daemon.borrow_mut().reconcile();
            daemon.borrow_mut().warm_icons();
        }
    }
}

/// Order displays left-to-right, then top-to-bottom, so a screen's index matches its physical layout
/// (the tab bar groups tabs by screen in index order, and tab cycling traverses the bar the same way).
fn sort_displays_by_position(screens: &mut [(u32, Rect, Rect)]) {
    screens.sort_by(|a, b| a.1.x.partial_cmp(&b.1.x).unwrap_or(std::cmp::Ordering::Equal).then(a.1.y.partial_cmp(&b.1.y).unwrap_or(std::cmp::Ordering::Equal)));
}
