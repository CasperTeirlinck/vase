//! The vase daemon: adopt windows as tabs, drive them from the modal prefix.

mod app_focus;
mod bar;
mod effects;
mod input;
mod pane_picker;
mod prompt;
mod reconcile;
mod switcher;

use std::collections::HashSet;
use std::path::PathBuf;

use crate::backend::Backend;
use crate::chrome::{Context, Deck, Painter, Position};
use crate::config::{AppFocus, Config};
use crate::geometry::Rect;
use crate::input::{NumberEntry, Switcher};
use crate::model::{apply, Command, Model};
use crate::registry::Registry;
use crate::tree::WindowId;

use pane_picker::{PendingLaunch, PickItem};
use prompt::PromptKind;
use switcher::SwitchItem;

/// How long to wait for a cross-display-moved window to settle before re-asserting its frame.
const REFRAME_SETTLE: std::time::Duration = std::time::Duration::from_millis(150);

/// Where the platform keeps vase's files. Either is `None` when there is no home directory to
/// resolve it against, which turns the corresponding persistence off rather than failing.
#[derive(Default, Clone)]
pub struct Paths {
    pub config: Option<PathBuf>,
    pub state: Option<PathBuf>,
}

pub struct Daemon<B: Backend, C: Painter> {
    pub model: Option<Model>,
    backend: B,
    /// Everything vase paints on top of the windows.
    chrome: Deck<C>,
    /// What vase knows about each adopted window, outside the layout model.
    windows: Registry,
    paths: Paths,
    /// Set by `prefix-q` or `:q`; the run loop polls it and exits.
    quit: bool,
    restored: bool,
    /// Windows shown by the last Render, to raise only on a tab switch.
    last_shown: HashSet<WindowId>,
    /// Windows just moved to a different monitor, re-framed once after they settle: some apps (e.g. cell-sized terminals) land a hair short until the frame is re-asserted.
    pending_reframe: Vec<(WindowId, Rect)>,
    reframe_deadline: Option<std::time::Instant>,
    /// App display names showing a notification badge; drives the red dot on their tabs.
    badges: HashSet<String>,
    badge_tick: u32,
    /// Whether a window is fullscreen, so the chrome hides instead of sitting over it.
    fullscreen: bool,
    /// Edge of the main display the tab bar sits on, and so which edge the layout gives up.
    bar_position: Position,
    /// Managed windows currently on another workspace.
    off_workspace: HashSet<WindowId>,
    /// Polls to skip OS-focus-following after our own focus command, so the window list's lag on a just-raised window doesn't flip focus back (a flicker).
    focus_cooldown: u32,
    /// Last observed OS-frontmost window, so focus-follow is edge-triggered (fires only on a real change).
    last_front: Option<WindowId>,
    /// The previously-focused window, for `prefix-l` (tmux last-window).
    pub last_focused: Option<WindowId>,
    /// The open window switcher's state, if shown.
    pub switcher: Option<Switcher<SwitchItem>>,
    /// Picker auto-opened over the focused empty pane: windows from other tabs plus launchable apps.
    pub pane_picker: Option<Switcher<PickItem>>,
    /// Launchable app names, discovered once at startup.
    apps: Vec<String>,
    /// Favorite app names, shown first in the app picker.
    favorites: Vec<String>,
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
    /// Index of the main display, where the tab bar lives.
    main_screen: usize,
    /// Full bounds of every display, for assigning a window to a monitor by its frame center.
    screens_cg: Vec<Rect>,
    /// Stable display id per screen index, so hotplug matches tabs to the same physical monitor.
    display_ids: Vec<u32>,
    /// Whether the prefix chord is armed; drives the prefix dot on the tab bar.
    pub prefix_armed: bool,
    /// Configurable global hotkeys that toggle focus to a specific app.
    app_hotkeys: Vec<AppFocus>,
    /// In-progress `prefix-<number>` tab selection.
    pub tab_entry: NumberEntry,
}

impl<B: Backend, C: Painter> Daemon<B, C> {
    /// Construct the daemon from the startup-computed window/display state.
    #[allow(clippy::too_many_arguments)]
    pub fn new(model: Model, backend: B, painter: C, windows: Registry, paths: Paths, main_screen: usize, screens_cg: Vec<Rect>, display_ids: Vec<u32>) -> Self {
        let config = paths.config.as_deref().map(Config::load).unwrap_or_default();
        crate::chrome::theme::set_theme(config.theme);
        crate::chrome::theme::set_mark(config.mark);
        let bar_position = config.bar_position.unwrap_or(backend.default_bar_position());
        let apps = backend.launchable_apps();
        Daemon {
            model: Some(model),
            backend,
            chrome: Deck::new(painter),
            windows,
            paths,
            quit: false,
            badges: HashSet::new(),
            badge_tick: 0,
            restored: false,
            last_shown: HashSet::new(),
            pending_reframe: Vec::new(),
            reframe_deadline: None,
            fullscreen: false,
            bar_position,
            off_workspace: HashSet::new(),
            focus_cooldown: 0,
            last_front: None,
            last_focused: None,
            switcher: None,
            pane_picker: None,
            apps,
            favorites: config.favorites,
            icon_warm: 0,
            pending_launch: None,
            prompt: None,
            resize_sticky: false,
            movetab_sticky: false,
            main_screen,
            screens_cg,
            prefix_armed: false,
            display_ids,
            app_hotkeys: config.app_focus,
            tab_entry: NumberEntry::default(),
        }
    }

    /// Whether the user asked vase to exit.
    pub fn quit_requested(&self) -> bool {
        self.quit
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
        // Draw the chrome first (it only reads the model, so it's instant); `execute`'s placements can block on a slow app, so do them last.
        self.refresh();
        self.execute(effects);
        self.save_state();
    }

    /// Redraw every surface from the current model. The one place that knows what a change redraws.
    pub fn refresh(&mut self) {
        // A fullscreen window owns the whole display; hide everything so nothing sits over it.
        if self.fullscreen {
            self.chrome.hide_all();
            return;
        }
        // The picker may open or close here, and the pane placeholders depend on whether it did.
        self.refresh_pane_picker();
        let Some(model) = &self.model else { return };
        let ctx = Context {
            windows: &self.windows,
            badges: &self.badges,
            off_workspace: &self.off_workspace,
            hotkeys: &self.app_hotkeys,
            main_screen: self.main_screen,
            bar_position: self.bar_position,
            prefix_armed: self.prefix_armed,
            prompt: self.prompt.as_ref().map(|(kind, buf)| format!("{}{buf}\u{258f}", kind.prefix())),
            picker_open: self.pane_picker.is_some(),
        };
        self.chrome.sync(model, &ctx);
    }

    /// Persist the layout so a restart restores it; stores each window's `(app, title)` to re-match after a reboot reassigns ids.
    pub(crate) fn save_state(&self) {
        let (Some(m), Some(path)) = (&self.model, &self.paths.state) else { return };
        let windows: Vec<crate::state::WindowIdentity> = m.all_windows().into_iter().map(|id| (id, self.windows.app(id).to_string(), self.windows.title(id).to_string())).collect();
        crate::state::save(path, m, &windows);
    }

    /// Resolve the adopted windows' app icons up front so the bar and switcher show them immediately.
    pub fn warm_window_icons(&mut self) {
        let apps: HashSet<String> = self.windows.iter().map(|(_, w)| w.app.clone()).collect();
        for app in apps {
            self.chrome.prewarm_icon(&app);
        }
    }

    /// Pre-warm a small batch of app icons per poll so the launch list is cached before the picker needs it.
    pub fn warm_icons(&mut self) {
        const BATCH: usize = 4;
        if self.icon_warm >= self.apps.len() {
            return;
        }
        let end = (self.icon_warm + BATCH).min(self.apps.len());
        // Split the borrow: the painter is behind `&mut self` while the batch reads `self.apps`.
        let Self { apps, chrome, icon_warm, .. } = self;
        for app in &apps[*icon_warm..end] {
            chrome.prewarm_icon(app);
        }
        self.icon_warm = end;
        if self.pane_picker.is_some() {
            self.render_pane_picker();
        }
    }
}
