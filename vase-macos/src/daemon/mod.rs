//! The vase daemon: adopt windows as tabs, drive them from the modal prefix,
//! restore every window on exit.

mod app_focus;
mod bar;
mod pane_picker;
mod prompt;
mod reconcile;
mod switcher;
mod util;

use std::collections::{HashMap, HashSet};

use objc2::MainThreadMarker;
use vase_core::backend::Backend;
use vase_core::geometry::Rect;
use vase_core::input::Switcher;
use vase_core::model::{apply, Command, Effect, Model};
use vase_core::tree::WindowId;

use crate::{FocusBorder, MacBackend, PaneOverlay, SwitcherView, TabBar};

use pane_picker::{PendingLaunch, PickItem};
use prompt::PromptKind;
use switcher::SwitchItem;
use util::discover_apps;

pub use util::{all_windows, clean_title, screen_of};

/// A visible stack bar's click map: its CG rect, its tab hit-ranges, and the
/// window id behind each range (index-aligned with the ranges).
pub type StackClick = (Rect, Vec<(f64, f64)>, Vec<WindowId>);

/// How long to wait for a cross-display-moved window to settle on its new
/// monitor before re-asserting its frame. Tune up if a window still lands short.
const REFRAME_SETTLE: std::time::Duration = std::time::Duration::from_millis(150);

pub struct Daemon {
    pub model: Option<Model>,
    backend: MacBackend,
    originals: HashMap<WindowId, Rect>,
    restored: bool,
    /// The window set shown by the last Render, to raise only on a tab switch.
    last_shown: HashSet<WindowId>,
    /// Last rect we placed each window at, to detect a cross-display move.
    placed: HashMap<WindowId, Rect>,
    /// Windows just moved to a different monitor, re-framed once after they
    /// settle (see `reframe_deadline`) — some apps (e.g. cell-sized terminals)
    /// land a hair short on the new display until the frame is re-asserted.
    pending_reframe: Vec<(WindowId, Rect)>,
    reframe_deadline: Option<std::time::Instant>,
    names: HashMap<WindowId, String>,
    titles: HashMap<WindowId, String>,
    /// App display names that currently show a Dock notification badge; drives
    /// the red dot on their tabs. Refreshed on a throttled poll.
    badges: HashSet<String>,
    badge_tick: u32,
    bar: TabBar,
    /// Pool of local powerline bars, one drawn in each visible stack's top strip.
    stack_bars: Vec<TabBar>,
    pub bar_rect: Option<Rect>,
    pub bar_ranges: Vec<(f64, f64)>,
    /// Per-visible-stack click map, used to route clicks on a local stack bar to
    /// `SelectStackWindow`.
    pub stack_click: Vec<StackClick>,
    /// Placeholder overlay drawing the current tab's empty panes.
    pane_overlay: PaneOverlay,
    /// Accent outline drawn around the focused pane when the tab is split.
    focus_border: FocusBorder,
    /// Polls to skip OS-focus-following after vase issues its own focus
    /// command, so a just-raised window that CGWindowList hasn't caught up to
    /// yet doesn't make the poll flip focus back (a tab-switch flicker).
    focus_cooldown: u32,
    /// Last observed OS-frontmost window, so focus-follow is edge-triggered (fires
    /// only when the frontmost actually changes, i.e. a real user click).
    last_front: Option<WindowId>,
    /// The previously-focused window, for `⌥a l` (tmux last-window).
    pub last_focused: Option<WindowId>,
    /// The open window switcher's state, if the overlay is currently shown.
    pub switcher: Option<Switcher<SwitchItem>>,
    switcher_view: SwitcherView,
    /// A `g` was pressed in the switcher's nav mode, awaiting a second `g` (gg).
    switcher_g_pending: bool,
    /// In-progress `<number>` index pick in the switcher, and its commit deadline
    /// (for double-digit entry), mirroring the tab-bar's `prefix-N`.
    switcher_entry: Option<usize>,
    switcher_entry_deadline: Option<std::time::Instant>,
    /// Picker auto-opened over the focused empty pane; lists windows from other
    /// tabs plus launchable apps. Shares `switcher_view`, rendered in the pane's
    /// rect via `show_in`.
    pub pane_picker: Option<Switcher<PickItem>>,
    /// `gg` pending for the pane picker's nav mode (see `switcher_g_pending`).
    pane_picker_g_pending: bool,
    /// In-progress `<number>` window pick in the pane picker + its commit
    /// deadline (double-digit entry, like the switcher's).
    pane_picker_entry: Option<usize>,
    pane_picker_entry_deadline: Option<std::time::Instant>,
    /// Launchable app names (file stems), discovered once at startup.
    apps: Vec<String>,
    /// Cursor into `apps` for background icon pre-warming (spread over polls so
    /// the picker never blocks resolving icons on its first render).
    icon_warm: usize,
    /// A launch spawned into the focused empty pane, awaiting its window.
    pub pending_launch: Option<PendingLaunch>,
    /// In-progress bar command line (rename or `:` command); `Some` while open.
    pub prompt: Option<(PromptKind, String)>,
    /// After a prefix resize, keep resizing on bare Shift-arrows (no re-prefix)
    /// until any other key; lets the user hold Shift and tap to resize in steps.
    pub resize_sticky: bool,
    /// After a prefix move-tab (⌥a </>), keep moving on bare Shift-,/Shift-.
    /// (no re-prefix) until any other key.
    pub movetab_sticky: bool,
    /// Index (into the model's screens) of the main/menu-bar display — where the
    /// tab bar lives.
    main_screen: usize,
    /// Full CG bounds of every display, for assigning a window to a monitor by
    /// the containing display of its frame center.
    screens_cg: Vec<Rect>,
    /// Stable display id per screen index, so hotplug matches tabs to the same
    /// physical monitor even when the index order changes.
    display_ids: Vec<u32>,
    /// Managed windows currently minimized to the Dock: kept as tabs but not
    /// placed/raised; selecting their tab restores them.
    minimized: HashSet<WindowId>,
    /// Whether the prefix chord is armed (awaiting a command key) — drives the
    /// tmux-style prefix dot on the far right of the tab bar.
    pub prefix_armed: bool,
    /// Main-thread marker, for querying displays (hotplug) from the reconcile loop.
    mtm: MainThreadMarker,
    /// Configurable global hotkeys that toggle focus to a specific app.
    app_hotkeys: Vec<crate::config::AppFocus>,
    /// In-progress `prefix-<number>` tab selection: the digits typed so far, and
    /// the instant it auto-commits (so multi-digit tab numbers get a brief window
    /// for the next digit).
    pub tab_entry: Option<usize>,
    tab_entry_deadline: Option<std::time::Instant>,
}

impl Daemon {
    /// Construct the daemon from the startup-computed window/display state; the
    /// app list, config hotkeys, and switcher overlay are set up internally.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        mtm: MainThreadMarker,
        model: Model,
        backend: MacBackend,
        originals: HashMap<WindowId, Rect>,
        names: HashMap<WindowId, String>,
        titles: HashMap<WindowId, String>,
        bar: TabBar,
        pane_overlay: PaneOverlay,
        focus_border: FocusBorder,
        main_screen: usize,
        screens_cg: Vec<Rect>,
        display_ids: Vec<u32>,
        minimized: HashSet<WindowId>,
    ) -> Self {
        Daemon {
            model: Some(model),
            backend,
            originals,
            titles,
            badges: HashSet::new(),
            badge_tick: 0,
            restored: false,
            last_shown: HashSet::new(),
            placed: HashMap::new(),
            pending_reframe: Vec::new(),
            reframe_deadline: None,
            names,
            bar,
            stack_bars: Vec::new(),
            bar_rect: None,
            bar_ranges: Vec::new(),
            stack_click: Vec::new(),
            pane_overlay,
            focus_border,
            focus_cooldown: 0,
            last_front: None,
            last_focused: None,
            switcher: None,
            switcher_view: SwitcherView::new(mtm),
            switcher_g_pending: false,
            switcher_entry: None,
            switcher_entry_deadline: None,
            pane_picker: None,
            pane_picker_g_pending: false,
            pane_picker_entry: None,
            pane_picker_entry_deadline: None,
            apps: discover_apps(),
            icon_warm: 0,
            pending_launch: None,
            prompt: None,
            resize_sticky: false,
            movetab_sticky: false,
            main_screen,
            screens_cg,
            minimized,
            prefix_armed: false,
            mtm,
            display_ids,
            app_hotkeys: crate::config::load(),
            tab_entry: None,
            tab_entry_deadline: None,
        }
    }

    pub fn dispatch(&mut self, cmd: Command) {
        // A focus command of our own (anything but the OS-focus sync) starts a
        // cooldown so the poll doesn't fight the raise while it settles.
        if !matches!(cmd, Command::SyncFocus(_)) {
            self.focus_cooldown = 3;
        }
        let old_focused = self.model.as_ref().unwrap().focused_window();
        let (model, effects) = apply(self.model.take().expect("model present"), cmd);
        self.model = Some(model);
        // Remember the window focus was on, so ⌥a l can jump back to it.
        let new_focused = self.model.as_ref().unwrap().focused_window();
        if new_focused != old_focused {
            if let Some(w) = old_focused {
                self.last_focused = Some(w);
            }
        }
        // Draw the overlays first: they only read the (already-updated) model, so
        // they appear instantly. `execute` issues the AX window placements, which
        // can block for a while on a slow app — doing it after keeps the pane
        // placeholder / focus border / bar from lagging behind the split.
        self.refresh_bar();
        self.refresh_stack_bars();
        self.refresh_pane_picker();
        self.refresh_panes();
        self.refresh_focus_border();
        self.execute(effects);
        self.save_state();
    }

    /// Persist the layout so a restart restores it. Small JSON write; best effort.
    /// Stores each window's `(app, title)` so a reboot (which reassigns window
    /// ids) can still re-match the saved tabs.
    pub(crate) fn save_state(&self) {
        if let Some(m) = &self.model {
            let windows: Vec<crate::state::WindowIdentity> = all_windows(m)
                .into_iter()
                .map(|id| {
                    (
                        id,
                        self.names.get(&id).cloned().unwrap_or_default(),
                        self.titles.get(&id).cloned().unwrap_or_default(),
                    )
                })
                .collect();
            crate::state::save(m, &windows);
        }
    }

    /// Resolve the adopted windows' app icons up front so the bar and switcher
    /// show them immediately (few apps → fast). The full app list is warmed
    /// lazily in the background by `warm_icons`.
    pub fn warm_window_icons(&self) {
        let mut seen = HashSet::new();
        for app in self.names.values() {
            if seen.insert(app.as_str()) {
                crate::overlay::prewarm_icon(app);
            }
        }
    }

    /// Pre-warm a small batch of app icons per poll so the launch list's icons
    /// are cached before the picker needs them (the render path is cache-only,
    /// so this never blocks it). Re-render an open picker as icons arrive.
    pub fn warm_icons(&mut self) {
        const BATCH: usize = 4;
        if self.icon_warm >= self.apps.len() {
            return;
        }
        let end = (self.icon_warm + BATCH).min(self.apps.len());
        for app in &self.apps[self.icon_warm..end] {
            crate::overlay::prewarm_icon(app);
        }
        self.icon_warm = end;
        if self.pane_picker.is_some() {
            self.render_pane_picker();
        }
    }

    pub fn execute(&mut self, effects: Vec<Effect>) {
        // A batch with a Render is a re-tile / bring-forward (tab switch, Raise,
        // switcher jump); a bare FocusWindow is a within-tab move. Only the
        // former co-surfaces the tab's other panes, so plain focus navigation
        // doesn't flick focus across an already-visible split.
        let bringing_forward = effects.iter().any(|e| matches!(e, Effect::Render(_)));
        // Selecting a minimized window's tab restores it: un-minimize the focus
        // target up front so the Render below can place and raise it.
        if let Some(w) = effects.iter().find_map(|e| match e {
            Effect::FocusWindow(w) => Some(*w),
            _ => None,
        }) {
            if self.minimized.remove(&w) {
                self.backend.set_minimized(w, false);
            }
        }
        for effect in effects {
            match effect {
                Effect::Render(placements) => {
                    let shown: HashSet<WindowId> = placements.iter().map(|(id, _)| *id).collect();
                    for (id, rect) in &placements {
                        // A minimized window keeps its tab but isn't placed.
                        if self.minimized.contains(id) {
                            continue;
                        }
                        // A window that just changed monitor gets re-framed once
                        // it settles: some apps land a hair short on the new
                        // display until the frame is re-asserted.
                        let moved_display = self
                            .placed
                            .get(id)
                            .is_some_and(|old| screen_of(*old, &self.screens_cg) != screen_of(*rect, &self.screens_cg));
                        self.placed.insert(*id, *rect);
                        self.backend.set_frame(*id, *rect);
                        if moved_display {
                            self.pending_reframe.push((*id, *rect));
                        }
                    }
                    if !self.pending_reframe.is_empty() {
                        self.reframe_deadline = Some(std::time::Instant::now() + REFRAME_SETTLE);
                    }
                    // Co-location, not off-screen parking: the current tab's
                    // windows tile the whole screen, so raising them above the
                    // rest hides the other tabs (no window ever slides off-screen).
                    // Raise only NEWLY-shown windows: re-raising an already-visible
                    // window on another display (e.g. a second Chrome window)
                    // fronts its app and flickers that display. Use `raise`, not
                    // `focus` — the trailing FocusWindow effect sets real focus.
                    if shown != self.last_shown {
                        for (id, _) in &placements {
                            if self.minimized.contains(id) || self.last_shown.contains(id) {
                                continue;
                            }
                            self.backend.raise(*id);
                        }
                        self.last_shown = shown;
                    }
                }
                // Bring EVERY currently-visible pane of the focused tab to the
                // front, then focus the target last — focusing one window must
                // never leave the rest of its split/stack layout covered (a tab's
                // layout is all-or-nothing; no holes). `focus` (SkyLight) raises
                // each SPECIFIC window, unlike `raise` which fronts a whole app
                // and can lift that app's off-tab windows over a sibling pane.
                // Only VISIBLE panes (in `last_shown`) are surfaced — a stack
                // shows one item at a time, so its hidden items stay put. Gated to
                // bring-forward batches so within-tab focus moves don't flick.
                Effect::FocusWindow(id) => {
                    if bringing_forward {
                        if let Some(tab) = self.model.as_ref().and_then(|m| m.focused_tab()) {
                            for sib in vase_core::tree::windows(&tab.root) {
                                if sib != id && self.last_shown.contains(&sib) {
                                    self.backend.focus(sib);
                                }
                            }
                        }
                    }
                    self.backend.focus(id);
                }
            }
        }
    }

    /// Re-assert the frames of windows that just moved to another monitor, once
    /// they've settled there (called each run-loop wake; fires after
    /// `REFRAME_SETTLE`). Mirrors what a manual focus-away-and-back did.
    pub fn tick_reframe(&mut self) {
        if self.reframe_deadline.is_some_and(|d| std::time::Instant::now() >= d) {
            self.reframe_deadline = None;
            for (id, rect) in std::mem::take(&mut self.pending_reframe) {
                if !self.minimized.contains(&id) {
                    self.backend.set_frame(id, rect);
                }
            }
        }
    }

    pub fn restore(&mut self) {
        if self.restored {
            return;
        }
        self.restored = true;
        self.save_state(); // capture the final layout before tearing down
        self.bar.hide();
        for bar in &self.stack_bars {
            bar.hide();
        }
        let originals: Vec<(WindowId, Rect)> =
            self.originals.iter().map(|(id, r)| (*id, *r)).collect();
        for (id, rect) in originals {
            self.backend.set_frame(id, rect);
        }
        println!("vase: restored {} windows.", self.originals.len());
    }
}
