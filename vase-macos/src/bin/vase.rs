//! The vase daemon: adopt windows as tabs, drive them from the modal prefix,
//! restore every window on exit. Quit with `⌥a q` or the kill chord
//! `Ctrl+Alt+Cmd+Esc`.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use objc2_app_kit::{NSApplication, NSEventMask};
use objc2_foundation::{NSDate, NSDefaultRunLoopMode};
use vase_core::backend::{manageable, Backend};
use vase_core::geometry::Rect;
use vase_core::input::{InputCommand, Key, KeyRouter, Mods};
use vase_core::model::{Command, Effect, Model};
use vase_core::tree::WindowId;
use vase_macos::daemon::{all_windows, screen_of, Daemon};
use vase_macos::keycodes::{VK_A, VK_E};
use vase_macos::{nsapp_init, EventTap, MacBackend, PaneOverlay, TabBar};

const VK_COMMA: u16 = 0x2B;
const VK_PERIOD: u16 = 0x2F;
const VK_Q: u16 = 0x0C;
const VK_BACKSLASH: u16 = 0x2A;
const VK_MINUS: u16 = 0x1B;
const VK_LEFT: u16 = 0x7B;
const VK_RIGHT: u16 = 0x7C;
const VK_DOWN: u16 = 0x7D;
const VK_UP: u16 = 0x7E;
const VK_L: u16 = 0x25;
const VK_H: u16 = 0x04;
const VK_J: u16 = 0x26;
const VK_K: u16 = 0x28;
const VK_W: u16 = 0x0D;
const VK_Z: u16 = 0x06;
const VK_X: u16 = 0x07;
const VK_T: u16 = 0x11;
const VK_S: u16 = 0x01;
const VK_C: u16 = 0x08;
const VK_LBRACKET: u16 = 0x21;
const VK_RBRACKET: u16 = 0x1E;
const VK_SEMICOLON: u16 = 0x29;

struct RestoreGuard(Rc<RefCell<Daemon>>);
impl Drop for RestoreGuard {
    fn drop(&mut self) {
        self.0.borrow_mut().restore();
    }
}

fn bindings() -> HashMap<Key, InputCommand> {
    let mut b = HashMap::new();
    b.insert(Key::plain(VK_PERIOD), InputCommand::StackNext);
    b.insert(Key::plain(VK_COMMA), InputCommand::StackPrev);
    b.insert(Key::plain(VK_L), InputCommand::LastTab);
    b.insert(Key::plain(VK_Q), InputCommand::Quit);
    b.insert(Key::plain(VK_BACKSLASH), InputCommand::SplitH);
    b.insert(Key::plain(VK_MINUS), InputCommand::SplitV);
    b.insert(Key::plain(VK_LEFT), InputCommand::FocusLeft);
    b.insert(Key::plain(VK_RIGHT), InputCommand::FocusRight);
    b.insert(Key::plain(VK_UP), InputCommand::FocusUp);
    b.insert(Key::plain(VK_DOWN), InputCommand::FocusDown);
    b.insert(Key::plain(VK_W), InputCommand::WindowSwitcher);
    b.insert(Key::plain(VK_Z), InputCommand::ZoomToggle);
    b.insert(Key::plain(VK_X), InputCommand::BreakPane);
    b.insert(Key::plain(VK_C), InputCommand::NewTab);
    b.insert(Key::plain(VK_S), InputCommand::Stackify);
    b.insert(Key::plain(VK_LBRACKET), InputCommand::StackFocusPrev);
    b.insert(Key::plain(VK_RBRACKET), InputCommand::StackFocusNext);
    b.insert(Key::plain(VK_T), InputCommand::Rename);
    let shift = Mods { shift: true, ..Mods::default() };
    // prefix-: — ":" is Shift-semicolon on a US layout.
    b.insert(Key { code: VK_SEMICOLON, mods: shift }, InputCommand::CommandLine);
    let cmd = Mods { cmd: true, ..Mods::default() };
    b.insert(Key { code: VK_LEFT, mods: shift }, InputCommand::ResizeLeft);
    b.insert(Key { code: VK_RIGHT, mods: shift }, InputCommand::ResizeRight);
    b.insert(Key { code: VK_UP, mods: shift }, InputCommand::ResizeUp);
    b.insert(Key { code: VK_DOWN, mods: shift }, InputCommand::ResizeDown);
    // Resize also on Shift-HJKL (vim), consistent with Shift-arrows.
    b.insert(Key { code: VK_H, mods: shift }, InputCommand::ResizeLeft);
    b.insert(Key { code: VK_J, mods: shift }, InputCommand::ResizeDown);
    b.insert(Key { code: VK_K, mods: shift }, InputCommand::ResizeUp);
    b.insert(Key { code: VK_L, mods: shift }, InputCommand::ResizeRight);
    // Move a pane: primarily on <mod>-HJKL (letters — the arrow-exchange
    // Karabiner rule can't touch them). Arrows are kept too so an armed
    // thumb-arrow doesn't leak to the terminal. Bind cmd/ctrl/alt for all three,
    // since the per-device modifier swaps land it differently per keyboard.
    let ctrl = Mods { ctrl: true, ..Mods::default() };
    let alt = Mods { alt: true, ..Mods::default() };
    for (code, mv) in [
        (VK_H, InputCommand::MoveLeft),
        (VK_L, InputCommand::MoveRight),
        (VK_K, InputCommand::MoveUp),
        (VK_J, InputCommand::MoveDown),
        (VK_LEFT, InputCommand::MoveLeft),
        (VK_RIGHT, InputCommand::MoveRight),
        (VK_UP, InputCommand::MoveUp),
        (VK_DOWN, InputCommand::MoveDown),
    ] {
        b.insert(Key { code, mods: cmd }, mv.clone());
        b.insert(Key { code, mods: ctrl }, mv.clone());
        b.insert(Key { code, mods: alt }, mv);
    }
    b.insert(Key { code: VK_COMMA, mods: shift }, InputCommand::MoveTabPrev);
    b.insert(Key { code: VK_PERIOD, mods: shift }, InputCommand::MoveTabNext);
    // ⌥a { / } (Shift-[ / ]) send the current tab to the left / right monitor.
    b.insert(Key { code: VK_LBRACKET, mods: shift }, InputCommand::MoveTabMonitorPrev);
    b.insert(Key { code: VK_RBRACKET, mods: shift }, InputCommand::MoveTabMonitorNext);
    // prefix-1..9 select the Nth tab in bar order.
    for n in 1..=9usize {
        if let Some(code) = vase_macos::keycodes::key_code_for_name(&n.to_string()) {
            b.insert(Key::plain(code), InputCommand::SelectBarTab(n));
        }
    }
    b
}

/// The ⌥e (nested-stack) binding set: identical to ⌥a except the tab-management
/// keys act on the focused stack (its local bar) instead of the screen's tabs.
fn bindings_nested() -> HashMap<Key, InputCommand> {
    let mut b = bindings();
    // . / , cycle the focused stack (mirrors ⌥a next/prev tab).
    b.insert(Key::plain(VK_PERIOD), InputCommand::StackFocusNext);
    b.insert(Key::plain(VK_COMMA), InputCommand::StackFocusPrev);
    // t renames the selected stack item (mirrors ⌥a rename).
    b.insert(Key::plain(VK_T), InputCommand::StackRename);
    // ⇧, / ⇧. reorder the selected stack item (mirrors ⌥a reorder tab).
    let shift = Mods { shift: true, ..Mods::default() };
    b.insert(Key { code: VK_COMMA, mods: shift }, InputCommand::StackMovePrev);
    b.insert(Key { code: VK_PERIOD, mods: shift }, InputCommand::StackMoveNext);
    // 1-9 select the Nth stack item (mirrors ⌥a select tab).
    for n in 1..=9usize {
        if let Some(code) = vase_macos::keycodes::key_code_for_name(&n.to_string()) {
            b.insert(Key::plain(code), InputCommand::StackSelectItem(n));
        }
    }
    b
}

fn main() {
    let mtm = objc2::MainThreadMarker::new().expect("main thread");
    nsapp_init(mtm);
    // Menu-bar item (vase icon → Settings / Quit); kept alive until main returns.
    let _status_bar = vase_macos::status::install(mtm);

    println!("vase: adopting each of your windows as its own tab.");
    println!("⌥a c  new tab  |  ⌥a .  next tab  |  ⌥a ,  prev tab  |  ⌥a 1-9  select tab  |  ⌥a t  rename  |  ⌥a :  command  |  ⌥a x  break pane  |  ⌥a q  quit");
    println!("commands: :q :rename <name> :close :split :vsplit :zoom :tab <n>");
    println!("⌥a \\  split → empty pane (right)   |   ⌥a -  split → empty pane (below)");
    println!("empty pane: picker opens — j/k pick, / search, ⏎ move a window in OR launch an app (⧉), esc cancels");
    println!("⌥a arrows focus | ⌥a z zoom | ⌥a ⇧arrows/⇧HJKL resize | ⌥a ⌘/⌃/⌥ + HJKL (or arrows) move pane | ⌥a ⇧,/. reorder tab | ⌥a {{/}} send tab to left/right monitor");
    println!("⌥a s  make/grow a stack (nested tabs)  |  click a stack's tab to select it");
    println!("⌥e = same keys, nested stack: ⌥e ./,  cycle  |  ⌥e 1-9  select  |  ⌥e t  rename  |  ⌥e ⇧,/.  reorder");
    println!("runs until ⌥a q or the kill chord: Ctrl+Alt+Cmd+Esc. all windows restored on exit.");

    let mut backend = MacBackend::new();
    let mut screens = vase_macos::overlay::all_screens(mtm);
    if screens.is_empty() {
        eprintln!("no screens — grant Accessibility (and Input Monitoring), then re-run.");
        std::process::exit(1);
    }
    // Order displays left-to-right (then top-to-bottom) so screen index matches
    // physical layout — the tab bar groups tabs by screen in this order, and
    // cycling/moving tabs traverses the bar in the same order.
    screens.sort_by(|a, b| {
        a.1.x.partial_cmp(&b.1.x).unwrap_or(std::cmp::Ordering::Equal).then(
            a.1.y.partial_cmp(&b.1.y).unwrap_or(std::cmp::Ordering::Equal),
        )
    });
    // Stable display id + full CG bounds per display. Ids match tabs to monitors
    // across hotplug; full bounds assign a window to a monitor by center.
    let display_ids: Vec<u32> = screens.iter().map(|(id, _, _)| *id).collect();
    let screens_cg: Vec<Rect> = screens.iter().map(|(_, full, _)| *full).collect();
    // The main display is the one at the CG global origin (0,0) — the menu-bar
    // display, and where the tab bar goes.
    let main_screen = screens_cg.iter().position(|r| r.x == 0.0 && r.y == 0.0).unwrap_or(0);

    // Usable rect per monitor = that display's visible frame (already excludes its
    // own menu bar and Dock); the main display also reserves a bottom strip for
    // the tab bar.
    let screen_rects: Vec<Rect> = screens
        .iter()
        .enumerate()
        .map(|(i, (_, _, vis))| {
            if i == main_screen {
                Rect::new(vis.x, vis.y, vis.w, vis.h - vase_macos::overlay::BAR_HEIGHT)
            } else {
                *vis
            }
        })
        .collect();

    let mut originals = HashMap::new();
    let mut names = HashMap::new();
    let mut titles = HashMap::new();
    let mut ids = Vec::new();
    let mut windows_with_screen = Vec::new();
    let onscreen = backend.list_windows();
    let onscreen_ids: HashSet<WindowId> = onscreen.iter().map(|w| w.id).collect();
    for w in &onscreen {
        if manageable(w) {
            originals.insert(w.id, w.frame);
            names.insert(w.id, w.app.clone());
            titles.insert(w.id, w.title.clone());
            windows_with_screen.push((w.id, screen_of(w.frame, &screens_cg)));
            ids.push(w.id);
        }
    }
    // Adopt windows that are already minimized at startup: scan the full window
    // list (all Spaces) and keep only the ones AX confirms are minimized, so we
    // don't pull in other Spaces' or background windows. They start in the
    // `minimized` set → shown as tabs but not placed until selected.
    let mut minimized = HashSet::new();
    for w in vase_macos::cg::all_windows() {
        if onscreen_ids.contains(&w.id) || !manageable(&w) {
            continue;
        }
        if backend.minimized_info(&w) == Some(true) {
            originals.insert(w.id, w.frame);
            names.insert(w.id, w.app.clone());
            titles.insert(w.id, w.title.clone());
            windows_with_screen.push((w.id, screen_of(w.frame, &screens_cg)));
            ids.push(w.id);
            minimized.insert(w.id);
        }
    }
    if ids.len() < 2 {
        eprintln!("need at least 2 manageable windows to demo tabs; found {}.", ids.len());
        eprintln!("(titles are empty without Screen Recording permission — grant it so windows qualify.)");
        std::process::exit(1);
    }
    println!("adopted {} windows as tabs. ⌥a . / , cycles them.", ids.len());

    // Restore the saved layout (tab order, names, per-monitor placement) if one
    // exists: adapt it to the current displays, prune windows that have since
    // closed, and adopt any live windows the saved layout didn't include. Window
    // ids are stable within a login session, so matching by id works (not across
    // a reboot). Otherwise start fresh.
    let model = match vase_macos::state::load() {
        Some((mut saved, identities)) => {
            saved.reconfigure(&screen_rects);
            // Re-match saved tabs to live windows. Pass 1: same session — the
            // saved window id is still live. Pass 2 (reboot): the id is gone, so
            // match by the saved (app, title), then by app alone. Build an
            // old-id -> live-id map; remap_windows renames survivors and prunes
            // saved windows with no live match.
            let live: Vec<(WindowId, String, String)> = windows_with_screen
                .iter()
                .map(|(id, _)| {
                    (
                        *id,
                        names.get(id).cloned().unwrap_or_default(),
                        titles.get(id).cloned().unwrap_or_default(),
                    )
                })
                .collect();
            let live_ids: HashSet<WindowId> = live.iter().map(|(id, _, _)| *id).collect();
            let saved_ids = all_windows(&saved);
            let idmap: HashMap<WindowId, (String, String)> =
                identities.into_iter().map(|(id, a, t)| (id, (a, t))).collect();

            let mut claimed: HashSet<WindowId> = HashSet::new();
            let mut map: HashMap<WindowId, WindowId> = HashMap::new();
            for old in &saved_ids {
                if live_ids.contains(old) {
                    map.insert(*old, *old);
                    claimed.insert(*old);
                }
            }
            for old in &saved_ids {
                if map.contains_key(old) {
                    continue;
                }
                let Some((app, title)) = idmap.get(old) else { continue };
                let pick = live
                    .iter()
                    .find(|(id, a, t)| {
                        !claimed.contains(id) && a.eq_ignore_ascii_case(app) && t == title
                    })
                    .or_else(|| {
                        live.iter()
                            .find(|(id, a, _)| !claimed.contains(id) && a.eq_ignore_ascii_case(app))
                    })
                    .map(|(id, _, _)| *id);
                if let Some(new) = pick {
                    map.insert(*old, new);
                    claimed.insert(new);
                }
            }
            saved.remap_windows(&map);
            // Live windows not matched to a saved tab become new tabs.
            for (w, si) in &windows_with_screen {
                if !claimed.contains(w) {
                    saved.add_window(*w, *si);
                }
            }
            println!("vase: restored saved layout.");
            saved
        }
        None => Model::adopt(&screen_rects, &windows_with_screen),
    };
    let bar = TabBar::new(mtm);
    let pane_overlay = PaneOverlay::new(mtm);
    let focus_border = vase_macos::FocusBorder::new(mtm);
    let daemon = Rc::new(RefCell::new(Daemon::new(
        mtm,
        model,
        backend,
        originals,
        names,
        titles,
        bar,
        pane_overlay,
        focus_border,
        main_screen,
        screens_cg,
        display_ids,
        minimized,
    )));
    // Armed now, before the tap exists: nothing is parked yet (adoption above
    // only reads frames), so a panic before install has nothing to undo — the
    // guard just becomes a no-op restore.
    let _guard = RestoreGuard(Rc::clone(&daemon));

    let daemon_cb = Rc::clone(&daemon);
    let on_command = Box::new(move |cmd: InputCommand| {
        use vase_core::focus::Direction;
        use vase_core::tree::Dir;
        let core_cmd = match cmd {
            InputCommand::StackNext => Some(Command::NextTab),
            InputCommand::StackPrev => Some(Command::PrevTab),
            InputCommand::NewTab => Some(Command::NewTab),
            InputCommand::SplitH => Some(Command::Split(Dir::Horizontal)),
            InputCommand::SplitV => Some(Command::Split(Dir::Vertical)),
            InputCommand::FocusLeft => Some(Command::Focus(Direction::Left)),
            InputCommand::FocusRight => Some(Command::Focus(Direction::Right)),
            InputCommand::FocusUp => Some(Command::Focus(Direction::Up)),
            InputCommand::FocusDown => Some(Command::Focus(Direction::Down)),
            // tmux last-window: jump back to the previously-focused window.
            InputCommand::LastTab => daemon_cb.borrow().last_focused.map(Command::Raise),
            InputCommand::Quit => {
                daemon_cb.borrow_mut().restore();
                vase_macos::request_quit();
                None
            }
            InputCommand::SendPrefix => None,
            InputCommand::WindowSwitcher => {
                daemon_cb.borrow_mut().open_switcher();
                None
            }
            InputCommand::BreakPane => {
                daemon_cb.borrow_mut().dispatch(Command::BreakPane);
                None
            }
            InputCommand::Stackify => Some(Command::Stackify),
            InputCommand::StackFocusPrev => Some(Command::StackCycle(-1)),
            InputCommand::StackFocusNext => Some(Command::StackCycle(1)),
            InputCommand::Rename => {
                daemon_cb.borrow_mut().start_rename();
                None
            }
            InputCommand::StackSelectItem(n) => Some(Command::StackSelect(n)),
            InputCommand::StackMovePrev => Some(Command::StackMove(-1)),
            InputCommand::StackMoveNext => Some(Command::StackMove(1)),
            InputCommand::StackRename => {
                daemon_cb.borrow_mut().start_stack_rename();
                None
            }
            InputCommand::CommandLine => {
                daemon_cb.borrow_mut().start_command();
                None
            }
            InputCommand::ZoomToggle => Some(Command::ToggleZoom),
            InputCommand::ResizeLeft => Some(Command::Resize(Direction::Left)),
            InputCommand::ResizeRight => Some(Command::Resize(Direction::Right)),
            InputCommand::ResizeUp => Some(Command::Resize(Direction::Up)),
            InputCommand::ResizeDown => Some(Command::Resize(Direction::Down)),
            InputCommand::MoveLeft => Some(Command::MoveWindow(Direction::Left)),
            InputCommand::MoveRight => Some(Command::MoveWindow(Direction::Right)),
            InputCommand::MoveUp => Some(Command::MoveWindow(Direction::Up)),
            InputCommand::MoveDown => Some(Command::MoveWindow(Direction::Down)),
            InputCommand::MoveTabPrev => Some(Command::MoveTab(-1)),
            InputCommand::MoveTabNext => Some(Command::MoveTab(1)),
            InputCommand::MoveTabMonitorPrev => Some(Command::MoveTabToScreen(-1)),
            InputCommand::MoveTabMonitorNext => Some(Command::MoveTabToScreen(1)),
            InputCommand::SelectBarTab(n) => {
                daemon_cb.borrow_mut().begin_tab_entry(n);
                None
            }
        };
        // A resize / move-tab arms sticky mode (bare Shift-arrows keep resizing,
        // bare Shift-,/. keep moving the tab); any other prefix command exits both.
        let is_resize = matches!(
            cmd,
            InputCommand::ResizeLeft
                | InputCommand::ResizeRight
                | InputCommand::ResizeUp
                | InputCommand::ResizeDown
        );
        let is_movetab = matches!(cmd, InputCommand::MoveTabPrev | InputCommand::MoveTabNext);
        if let Some(c) = core_cmd {
            daemon_cb.borrow_mut().dispatch(c);
        }
        let mut d = daemon_cb.borrow_mut();
        d.resize_sticky = is_resize;
        d.movetab_sticky = is_movetab;
    });

    let daemon_click = Rc::clone(&daemon);
    let on_click = Box::new(move |(px, py): (f64, f64)| -> bool {
        let mut d = daemon_click.borrow_mut();
        // A click on a local stack bar selects that stack item.
        let hit = d.stack_click.iter().find_map(|(rect, ranges, ids)| {
            if px < rect.x || px >= rect.x + rect.w || py < rect.y || py >= rect.y + rect.h {
                return None;
            }
            let local = px - rect.x;
            ranges.iter().position(|(a, b)| local >= *a && local < *b).and_then(|i| ids.get(i).copied())
        });
        if let Some(wid) = hit {
            d.dispatch(Command::SelectStackWindow(wid));
            return true;
        }
        // Otherwise, a click on the main tab bar selects that top-level tab.
        let Some(rect) = d.bar_rect else { return false };
        if px < rect.x || px >= rect.x + rect.w || py < rect.y || py >= rect.y + rect.h {
            return false;
        }
        let local = px - rect.x;
        let index = d.bar_ranges.iter().position(|(a, b)| local >= *a && local < *b);
        match index.and_then(|i| d.flat_tab_to_screen(i)) {
            Some((si, ti)) => {
                d.dispatch(Command::SelectScreenTab(si, ti));
                true
            }
            None => false,
        }
    });

    let daemon_key = Rc::clone(&daemon);
    let on_key_intercept = Box::new(move |key: vase_core::input::Key| -> bool {
        let mut d = daemon_key.borrow_mut();
        // While a launch is in flight the pane is modal: only Esc (cancel,
        // collapsing the pane) is handled; every other key is swallowed.
        if d.pending_launch.is_some() {
            const VK_ESC: u16 = 0x35;
            if key.code == VK_ESC {
                d.pending_launch = None;
                d.dispatch(Command::CloseFocusedPane);
            }
            return true;
        }
        if d.prompt.is_some() {
            return d.prompt_key(key);
        }
        if d.switcher.is_some() {
            return d.switcher_key(key);
        }
        if d.pane_picker.is_some() {
            return d.pane_picker_key(key);
        }
        // Mid `prefix-<number>` entry: capture further digits (the router only
        // routes the first key after the prefix) so multi-digit tab numbers work.
        if d.tab_entry.is_some() {
            return d.tab_entry_key(key);
        }
        // Configurable global app-focus hotkeys (e.g. Ctrl-` → Ghostty). Direct
        // chords, not behind the prefix; modal overlays above take precedence.
        if let Some(app) = d.app_hotkey(key) {
            d.toggle_app_focus(&app);
            return true;
        }
        // Sticky resize: after a prefix resize, keep resizing on bare Shift-arrows
        // (no re-prefix) until any other key.
        if d.resize_sticky {
            use vase_core::focus::Direction;
            let shift_only = key.mods.shift && !key.mods.cmd && !key.mods.ctrl && !key.mods.alt;
            let dir = match key.code {
                VK_LEFT | VK_H => Some(Direction::Left),
                VK_RIGHT | VK_L => Some(Direction::Right),
                VK_UP | VK_K => Some(Direction::Up),
                VK_DOWN | VK_J => Some(Direction::Down),
                _ => None,
            };
            if let (true, Some(dir)) = (shift_only, dir) {
                d.dispatch(Command::Resize(dir));
                return true;
            }
            d.resize_sticky = false;
        }
        // Sticky move-tab: after a prefix ⌥a </>, keep moving on bare Shift-,/.
        if d.movetab_sticky {
            let shift_only = key.mods.shift && !key.mods.cmd && !key.mods.ctrl && !key.mods.alt;
            let offset = match key.code {
                VK_COMMA => Some(-1),
                VK_PERIOD => Some(1),
                _ => None,
            };
            if let (true, Some(offset)) = (shift_only, offset) {
                d.dispatch(Command::MoveTab(offset));
                return true;
            }
            d.movetab_sticky = false;
        }
        false
    });

    let daemon_arm = Rc::clone(&daemon);
    let on_arm = Box::new(move |armed: bool| {
        let mut d = daemon_arm.borrow_mut();
        d.prefix_armed = armed;
        d.refresh_bar(); // recolor the prefix dot
    });

    let router =
        KeyRouter::new(Key::alt(VK_A), bindings()).with_prefix(Key::alt(VK_E), bindings_nested());
    let tap = match EventTap::install(router, on_command, on_click, on_key_intercept, on_arm) {
        Some(t) => t,
        None => {
            eprintln!("could not install event tap — grant Accessibility AND Input Monitoring.");
            // Nothing has been parked yet (no render has run), so it is safe
            // that process::exit skips the RestoreGuard destructor here.
            std::process::exit(1);
        }
    };

    // Only park windows once the tap is confirmed live, so every reachable
    // exit path from here on is covered by the run loop + restore below.
    // Co-locate every adopted window at the screen rect once, then raise the
    // first tab on top. From here, cycling tabs is a pure raise (⌥a . / ,
    // emit only FocusWindow) — windows never move again until quit/restore.
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
    daemon.borrow_mut().refresh_bar();
    daemon.borrow_mut().refresh_stack_bars();
    daemon.borrow_mut().refresh_panes();

    // Persistent: runs until ⌥a q, the kill chord, or a crash. Nothing is parked
    // off-screen, so an abrupt exit leaves windows visible, not lost. Wake every
    // ~25 ms so the prefix-number timeout stays crisp, but run the expensive
    // reconcile (window-list diff) only every ~4th wake (~100 ms).
    // Drive AppKit's own event loop (like `[NSApp run]`, but with our timed
    // ticks): block up to ~25 ms for the next event, dispatch it, then drain any
    // others. This is what lets the status-item menu open and act; `nextEvent…`
    // also services the CFRunLoop (the event tap is on common modes). Exit is a
    // flag, not the run-loop's stopped state — mixing the two is what made the
    // daemon quit on launch.
    let app = NSApplication::sharedApplication(mtm);
    let mode = unsafe { NSDefaultRunLoopMode };
    let mut wake = 0u32;
    while !vase_macos::should_quit() {
        let deadline = NSDate::dateWithTimeIntervalSinceNow(0.025);
        if let Some(event) = app.nextEventMatchingMask_untilDate_inMode_dequeue(
            NSEventMask::Any,
            Some(&deadline),
            mode,
            true,
        ) {
            app.sendEvent(&event);
            let past = NSDate::distantPast();
            while let Some(event) = app.nextEventMatchingMask_untilDate_inMode_dequeue(
                NSEventMask::Any,
                Some(&past),
                mode,
                true,
            ) {
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
        daemon.borrow_mut().tick_switcher_entry();
        daemon.borrow_mut().tick_pane_picker_entry();
        daemon.borrow_mut().tick_reframe();
        wake += 1;
        if wake % 4 == 0 {
            daemon.borrow_mut().reconcile();
            daemon.borrow_mut().warm_icons();
        }
    }
    daemon.borrow_mut().restore();
    println!("vase daemon: stopped, windows restored.");
}

#[cfg(test)]
#[path = "vase_test.rs"]
mod tests;
