//! The vase daemon: adopt windows as tabs, drive them from the modal prefix.

mod app_focus;
mod bar;
mod effects;
mod input;
mod pane_picker;
mod prompt;
mod reconcile;
mod switcher;
mod util;

use std::collections::HashSet;

use objc2::MainThreadMarker;
use vase_core::geometry::Rect;
use vase_core::input::{NumberEntry, Switcher};
use vase_core::model::{apply, Command, Model};
use vase_core::tree::WindowId;

use crate::overlay::{Chrome, Overlays};
use crate::registry::Registry;
use crate::MacBackend;

use pane_picker::{PendingLaunch, PickItem};
use prompt::PromptKind;
use switcher::SwitchItem;
use util::discover_apps;

pub use util::{all_windows, app_matches, clean_title, screen_of};

/// How long to wait for a cross-display-moved window to settle before re-asserting its frame.
const REFRAME_SETTLE: std::time::Duration = std::time::Duration::from_millis(150);

pub struct Daemon {
    pub model: Option<Model>,
    backend: MacBackend,
    /// What vase knows about each adopted window, outside the layout model.
    windows: Registry,
    restored: bool,
    /// Windows shown by the last Render, to raise only on a tab switch.
    last_shown: HashSet<WindowId>,
    /// Windows just moved to a different monitor, re-framed once after they settle: some apps (e.g. cell-sized terminals) land a hair short until the frame is re-asserted.
    pending_reframe: Vec<(WindowId, Rect)>,
    reframe_deadline: Option<std::time::Instant>,
    /// App display names showing a Dock notification badge; drives the red dot on their tabs.
    badges: HashSet<String>,
    badge_tick: u32,
    /// Everything vase paints on top of the windows.
    overlays: Overlays,
    /// Whether a window is fullscreen, so the overlays hide instead of sitting over it.
    fullscreen: bool,
    /// Polls to skip OS-focus-following after our own focus command, so CGWindowList's lag on a just-raised window doesn't flip focus back (a flicker).
    focus_cooldown: u32,
    /// Last observed OS-frontmost window, so focus-follow is edge-triggered (fires only on a real change).
    last_front: Option<WindowId>,
    /// The previously-focused window, for `prefix-l` (tmux last-window).
    pub last_focused: Option<WindowId>,
    /// The open window switcher's state, if shown.
    pub switcher: Option<Switcher<SwitchItem>>,
    /// Picker auto-opened over the focused empty pane: windows from other tabs plus launchable apps.
    pub pane_picker: Option<Switcher<PickItem>>,
    /// Launchable app names (file stems), discovered once at startup.
    apps: Vec<String>,
    /// Cursor into `apps` for background icon pre-warming, spread over polls.
    icon_warm: usize,
    /// A launch spawned into the focused empty pane, awaiting its window.
    pub pending_launch: Option<PendingLaunch>,
    /// In-progress bar command line (rename or `:` command).
    pub prompt: Option<(PromptKind, String)>,
    /// After a prefix resize, keep resizing on bare Shift-arrows until any other key.
    resize_sticky: bool,
    /// After a prefix move-tab, keep moving on bare Shift-,/Shift-. until any other key.
    movetab_sticky: bool,
    /// Index of the main/menu-bar display, where the tab bar lives.
    main_screen: usize,
    /// Full CG bounds of every display, for assigning a window to a monitor by its frame center.
    screens_cg: Vec<Rect>,
    /// Stable display id per screen index, so hotplug matches tabs to the same physical monitor.
    display_ids: Vec<u32>,
    /// Whether the prefix chord is armed; drives the prefix dot on the tab bar.
    pub prefix_armed: bool,
    /// Main-thread marker, for querying displays from the reconcile loop.
    mtm: MainThreadMarker,
    /// Configurable global hotkeys that toggle focus to a specific app.
    app_hotkeys: Vec<crate::config::AppFocus>,
    /// In-progress `prefix-<number>` tab selection.
    pub tab_entry: NumberEntry,
}

impl Daemon {
    /// Construct the daemon from the startup-computed window/display state.
    #[allow(clippy::too_many_arguments)]
    pub fn new(mtm: MainThreadMarker, model: Model, backend: MacBackend, windows: Registry, main_screen: usize, screens_cg: Vec<Rect>, display_ids: Vec<u32>) -> Self {
        Daemon {
            model: Some(model),
            backend,
            windows,
            badges: HashSet::new(),
            badge_tick: 0,
            restored: false,
            last_shown: HashSet::new(),
            pending_reframe: Vec::new(),
            reframe_deadline: None,
            overlays: Overlays::new(mtm),
            fullscreen: false,
            focus_cooldown: 0,
            last_front: None,
            last_focused: None,
            switcher: None,
            pane_picker: None,
            apps: discover_apps(),
            icon_warm: 0,
            pending_launch: None,
            prompt: None,
            resize_sticky: false,
            movetab_sticky: false,
            main_screen,
            screens_cg,
            prefix_armed: false,
            mtm,
            display_ids,
            app_hotkeys: crate::config::load(),
            tab_entry: NumberEntry::default(),
        }
    }

    pub fn dispatch(&mut self, cmd: Command) {
        // Our own focus command starts a cooldown so the poll doesn't fight the raise while it settles.
        if !matches!(cmd, Command::SyncFocus(_)) {
            self.focus_cooldown = 3;
        }
        let old_focused = self.model.as_ref().unwrap().focused_window();
        let (model, effects) = apply(self.model.take().unwrap(), cmd);
        self.model = Some(model);
        // Remember the window focus was on, so prefix-l can jump back to it.
        let new_focused = self.model.as_ref().unwrap().focused_window();
        if new_focused != old_focused {
            if let Some(w) = old_focused {
                self.last_focused = Some(w);
            }
        }
        // Draw overlays first (they only read the model, so they're instant); `execute`'s AX placements can block on a slow app, so do them last.
        self.refresh();
        self.execute(effects);
        self.save_state();
    }

    /// Redraw every overlay from the current model. The one place that knows what a change redraws.
    pub fn refresh(&mut self) {
        // A fullscreen window owns the whole display; hide every overlay so nothing sits over it.
        if self.fullscreen {
            self.overlays.hide_all();
            return;
        }
        // The picker may open or close here, and the pane placeholders depend on whether it did.
        self.refresh_pane_picker();
        let Some(model) = &self.model else { return };
        let chrome = Chrome {
            windows: &self.windows,
            badges: &self.badges,
            hotkeys: &self.app_hotkeys,
            main_screen: self.main_screen,
            prefix_armed: self.prefix_armed,
            prompt: self.prompt.as_ref().map(|(kind, buf)| format!("{}{buf}\u{258f}", kind.prefix())),
            picker_open: self.pane_picker.is_some(),
        };
        self.overlays.sync(model, &chrome);
    }

    /// Persist the layout so a restart restores it; stores each window's `(app, title)` to re-match after a reboot reassigns ids.
    pub(crate) fn save_state(&self) {
        if let Some(m) = &self.model {
            let windows: Vec<crate::state::WindowIdentity> = all_windows(m).into_iter().map(|id| (id, self.windows.app(id).to_string(), self.windows.title(id).to_string())).collect();
            crate::state::save(m, &windows);
        }
    }

    /// Resolve the adopted windows' app icons up front so the bar and switcher show them immediately.
    pub fn warm_window_icons(&self) {
        let mut seen = HashSet::new();
        for (_, w) in self.windows.iter() {
            if seen.insert(w.app.as_str()) {
                crate::overlay::prewarm_icon(&w.app);
            }
        }
    }

    /// Pre-warm a small batch of app icons per poll so the launch list is cached before the picker needs it.
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
}
