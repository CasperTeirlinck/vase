//! Every surface vase paints on top of the windows, behind one redraw call.
//!
//! The surfaces have to agree with each other: the stack bars sit inside the rects the pane
//! placeholders leave, the focus border traces the pane the tab bar highlights, and the click maps
//! are only valid for what was last drawn. Redrawing them one at a time from the call site got that
//! wrong three different ways, so `sync` is the only way in.

use std::collections::HashSet;

use crate::config::AppFocus;
use crate::geometry::Rect;
use crate::model::{Command, Model};
use crate::registry::{app_matches, clean_title, Registry};
use crate::tree::WindowId;

use super::bar::{Bar, BarTab, Hits};
use super::paint::{BarHits, ListAt, Painter, SwitchRow};
use super::Position;

/// A drawn stack bar's click map: its rect, per-item hit ranges, and what each range selects.
pub(crate) type ClickMap = (Rect, Hits, Vec<WindowId>);

/// The drawn tab bar's click map: its rect, where its pieces landed, and the apps its trailing icons stand for.
pub(crate) struct BarMap {
    pub rect: Rect,
    pub hits: BarHits,
    pub apps: Vec<String>,
}

/// What a click on the chrome asks for.
#[derive(Debug, Clone, PartialEq)]
pub enum Click {
    /// A model edit: selecting a tab or a stack item.
    Command(Command),
    /// A trailing icon: bring that windowless app back.
    Activate(String),
}

/// What the chrome needs on top of the model to draw a frame.
pub struct Context<'a> {
    pub windows: &'a Registry,
    /// Apps showing a notification badge; drives the red dot on their tabs.
    pub badges: &'a HashSet<String>,
    /// Managed windows on another workspace; drives the marker on their tabs and rows.
    pub off_workspace: &'a HashSet<WindowId>,
    /// Apps with a focus-toggle hotkey; their tabs get a marker.
    pub hotkeys: &'a [AppFocus],
    /// Running apps with no window, drawn as bare icons after the last tab.
    pub windowless: &'a [String],
    /// Display the tab bar lives on.
    pub main_screen: usize,
    /// Edge of that display the tab bar sits on.
    pub bar_position: Position,
    /// Outline the focused pane of a split tab.
    pub focus_border: bool,
    pub prefix_armed: bool,
    /// Command line contents, drawn in place of the tabs while it is open.
    pub prompt: Option<String>,
    /// The picker covers the focused empty pane, so no placeholder is drawn there.
    pub picker_open: bool,
}

pub struct Deck<C: Painter> {
    painter: C,
    /// Click maps from the last `sync`, in the order the bars were drawn.
    bar_map: Option<BarMap>,
    stack_hits: Vec<ClickMap>,
}

impl<C: Painter> Deck<C> {
    pub fn new(painter: C) -> Deck<C> {
        Deck { painter, bar_map: None, stack_hits: Vec::new() }
    }

    /// Redraw every surface from the model. Nothing else may draw them.
    pub fn sync(&mut self, model: &Model, ctx: &Context) {
        self.sync_bar(model, ctx);
        self.sync_stack_bars(model, ctx);
        self.sync_panes(model, ctx);
        self.sync_focus_border(model, ctx);
    }

    /// What a click resolves to, against the click maps left by the last `sync`.
    pub fn hit(&self, model: &Model, px: f64, py: f64) -> Option<Click> {
        route_click(model, self.bar_map.as_ref(), &self.stack_hits, px, py)
    }

    pub fn list(&mut self, at: ListAt, header: &str, rows: &[SwitchRow], selected: usize) {
        self.painter.list(at, header, rows, selected);
    }

    pub fn hide_list(&mut self) {
        self.painter.hide_list();
    }

    /// Draw the shortcut sheet, centered on `screen`.
    pub fn help(&mut self, screen: Rect) {
        self.painter.help(&super::help::layout(screen));
    }

    pub fn hide_help(&mut self) {
        self.painter.hide_help();
    }

    pub fn hide_bars(&mut self) {
        self.painter.hide_bars();
    }

    pub fn hide_all(&mut self) {
        self.painter.hide_all();
    }

    pub fn prewarm_icon(&mut self, app: &str) {
        self.painter.prewarm_icon(app);
    }

    fn sync_bar(&mut self, model: &Model, ctx: &Context) {
        let screen = model.screens[ctx.main_screen].rect;
        // The bar's rect: the reserved strip on the far side of the content rect, full width.
        let strip = super::bar_height();
        let bar_y = match ctx.bar_position {
            Position::Top => screen.y - strip,
            Position::Bottom => screen.y + screen.h,
        };
        let bar_rect = Rect::new(screen.x, bar_y, screen.w, strip);
        // While the command line is open it owns the bar; no tabs, and no click targets.
        if let Some(line) = &ctx.prompt {
            self.painter.prompt(bar_rect, line);
            self.bar_map = None;
            return;
        }
        let (tabs, selected) = model.bar_tabs();
        if tabs.is_empty() {
            self.painter.hide_bar();
            self.bar_map = None;
            return;
        }
        let bar_tabs: Vec<BarTab> = tabs
            .iter()
            .enumerate()
            .map(|(i, (windows, rep, name))| {
                let icons: Vec<String> = windows.iter().map(|id| ctx.windows.app(*id).to_string()).collect();
                let badges: Vec<bool> = icons.iter().map(|a| ctx.badges.contains(a)).collect();
                let hotkey = icons.iter().any(|a| ctx.hotkeys.iter().any(|h| app_matches(a, &h.app)));
                let app = rep.map(|id| ctx.windows.app(id).to_string()).unwrap_or_default();
                let label = match name {
                    // A whitespace-only custom name renders as just the icon.
                    Some(n) if n.trim().is_empty() => String::new(),
                    Some(n) => n.clone(),
                    None => {
                        let ct = clean_title(rep.map(|id| ctx.windows.title(id)).unwrap_or_default(), &app);
                        if ct.is_empty() {
                            app
                        } else {
                            ct
                        }
                    }
                };
                // Dim tabs not on the focused monitor; the number is the tab's `prefix-N` shortcut.
                let dim = model.screen_tab(i).is_some_and(|(si, _)| si != model.focused_screen);
                let off_workspace = windows.iter().any(|id| ctx.off_workspace.contains(id));
                BarTab { icons, badges, label, zoomed: model.zoomed && i == selected, number: i + 1, dim, off_workspace, hotkey }
            })
            .collect();
        let bar = Bar { rect: bar_rect, tabs: &bar_tabs, apps: ctx.windowless, selected, main: true, armed: ctx.prefix_armed };
        let hits = self.painter.bar(&bar);
        self.bar_map = Some(BarMap { rect: bar_rect, hits, apps: ctx.windowless.to_vec() });
    }

    fn sync_stack_bars(&mut self, model: &Model, ctx: &Context) {
        let stacks = model.stacks();
        #[allow(clippy::type_complexity)]
        let bars: Vec<(Rect, Vec<BarTab>, usize)> = stacks
            .iter()
            .map(|stack| {
                let tabs: Vec<BarTab> = stack
                    .items
                    .iter()
                    .enumerate()
                    .map(|(i, id)| {
                        let app = ctx.windows.app(*id).to_string();
                        // A custom name overrides the window title.
                        let label = match model.names.get(id) {
                            Some(name) => name.clone(),
                            None => {
                                let ct = clean_title(ctx.windows.title(*id), &app);
                                if ct.is_empty() {
                                    app.clone()
                                } else {
                                    ct
                                }
                            }
                        };
                        let badged = ctx.badges.contains(&app);
                        let off_workspace = ctx.off_workspace.contains(id);
                        BarTab { icons: vec![app], badges: vec![badged], label, zoomed: false, number: i + 1, dim: false, off_workspace, hotkey: false }
                    })
                    .collect();
                (Rect::new(stack.rect.x, stack.rect.y, stack.rect.w, super::bar_height()), tabs, stack.selected)
            })
            .collect();
        let strips: Vec<Bar> = bars.iter().map(|(rect, tabs, selected)| Bar { rect: *rect, tabs, apps: &[], selected: *selected, main: false, armed: false }).collect();
        let hits = self.painter.stack_bars(&strips);
        self.stack_hits = strips.iter().zip(hits).zip(&stacks).map(|((strip, hits), stack)| (strip.rect, hits, stack.items.clone())).collect();
    }

    fn sync_panes(&mut self, model: &Model, ctx: &Context) {
        let panes = model.empty_panes();
        let boxes: Vec<(Rect, bool)> = if ctx.picker_open { panes.into_iter().filter(|(_, focused)| !focused).collect() } else { panes };
        self.painter.panes(&boxes);
    }

    /// Outline the focused pane when the tab is split and the pane holds a window.
    fn sync_focus_border(&mut self, model: &Model, ctx: &Context) {
        let split = ctx.focus_border && model.current_pane_count() > 1 && model.focused_window().is_some();
        self.painter.focus_border(split.then(|| model.focused_pane_rect()).flatten());
    }
}

/// Free of `Deck` so the routing arithmetic is testable without a painter.
pub(crate) fn route_click(model: &Model, bar: Option<&BarMap>, stacks: &[ClickMap], px: f64, py: f64) -> Option<Click> {
    // A stack bar is drawn over the tab it belongs to, so it gets first refusal.
    for (rect, ranges, ids) in stacks {
        if let Some(id) = hit_range(*rect, ranges, px, py).and_then(|i| ids.get(i).copied()) {
            return Some(Click::Command(Command::SelectStackWindow(id)));
        }
    }
    let bar = bar?;
    if let Some(flat) = hit_range(bar.rect, &bar.hits.tabs, px, py) {
        let (si, ti) = model.screen_tab(flat)?;
        return Some(Click::Command(Command::SelectScreenTab(si, ti)));
    }
    let app = hit_range(bar.rect, &bar.hits.apps, px, py).and_then(|i| bar.apps.get(i))?;
    Some(Click::Activate(app.clone()))
}

/// Index of the horizontal range a point falls in, if the point is inside `rect` at all.
fn hit_range(rect: Rect, ranges: &[(f64, f64)], px: f64, py: f64) -> Option<usize> {
    if px < rect.x || px >= rect.x + rect.w || py < rect.y || py >= rect.y + rect.h {
        return None;
    }
    let local = px - rect.x;
    ranges.iter().position(|(a, b)| local >= *a && local < *b)
}
