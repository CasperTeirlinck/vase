//! The Direct2D painter: vase's chrome, drawn in the Fluent idiom.

mod fluent;
mod gpu;
mod icons;
pub(crate) mod paths;
mod system;

use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;
use windows::Win32::Graphics::Direct2D::{ID2D1Brush, ID2D1DeviceContext, D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_ELLIPSE, D2D1_INTERPOLATION_MODE_LINEAR, D2D1_ROUNDED_RECT};
use windows::Win32::Graphics::DirectWrite::DWRITE_TEXT_METRICS;
use windows_numerics::Vector2;

use vase_core::chrome::bar::{self, Bar, Hits, Run};
use vase_core::chrome::powerline::{self, BarLayout, LeadGlyph, DOT_D, TAB_ICON};
use vase_core::chrome::theme::{mark, style, Role, Style, PANE_PAD, PANE_RADIUS};
use vase_core::chrome::BarHits;
use vase_core::chrome::{bar_height, ListAt, Painter, SwitchRow, FONT_SIZE};
use vase_core::geometry::{bbox, Rect};

use gpu::{color, Gpu, Surface};
use icons::Icons;

/// Row height in a list.
const ROW_H: f64 = 28.0;
const LIST_WIDTH: f64 = 640.0;
// Cap the centered list at this fraction of the screen height before it starts scrolling.
const LIST_MAX_SCREEN_FRAC: f64 = 0.85;

pub struct D2DPainter {
    gpu: Gpu,
    bar: Surface,
    /// Pool of local bars, one per visible stack.
    stack_bars: Vec<Surface>,
    panes: Surface,
    focus: Surface,
    list: Surface,
    icons: Icons,
}

/// A bar laid out in the style the theme names, ready for that style's own painter.
enum Laid {
    Fluent(fluent::Strip),
    Powerline(BarLayout),
}

impl Laid {
    fn hits(&self) -> BarHits {
        match self {
            // The Fluent strip trails no icons: Windows reports no windowless apps to draw.
            Laid::Fluent(strip) => BarHits { tabs: strip.hits(), apps: Vec::new() },
            Laid::Powerline(layout) => BarHits { tabs: layout.hits(), apps: layout.app_hits() },
        }
    }

    /// The strip alone, for the command line to draw over.
    fn bare(self) -> Laid {
        match self {
            Laid::Fluent(strip) => Laid::Fluent(strip.bare()),
            Laid::Powerline(layout) => Laid::Powerline(layout.bare()),
        }
    }
}

impl D2DPainter {
    pub fn new() -> windows::core::Result<D2DPainter> {
        let gpu = Gpu::new()?;
        Ok(D2DPainter { bar: Surface::new(&gpu)?, stack_bars: Vec::new(), panes: Surface::new(&gpu)?, focus: Surface::new(&gpu)?, list: Surface::new(&gpu)?, icons: Icons::default(), gpu })
    }

    /// Lay a bar out in the theme's style, against DirectWrite's own text metrics.
    fn lay_out(&self, bar: &Bar) -> Laid {
        let measure = |text: &str, size: f64| self.gpu.measure(text, size);
        match style() {
            Style::Native => Laid::Fluent(fluent::layout(bar, &measure)),
            Style::Powerline => Laid::Powerline(powerline::layout(bar, &mark(), &measure)),
        }
    }

    /// Paint one laid-out bar into `surface`, in the style it was laid out in. `prompt` is the command
    /// line's text, which goes in the same pass: a second pass over the surface would clear the strip.
    fn paint_bar(gpu: &Gpu, icons: &Icons, surface: &mut Surface, laid: &Laid, prompt: Option<&str>, apps: &[String]) {
        match laid {
            Laid::Fluent(strip) => fluent::paint(gpu, icons, surface, strip, prompt),
            Laid::Powerline(layout) => Self::paint_powerline(gpu, icons, surface, layout, prompt, apps),
        }
    }

    /// Paint one laid-out powerline bar into `surface`.
    fn paint_powerline(gpu: &Gpu, icons: &Icons, surface: &mut Surface, layout: &BarLayout, prompt: Option<&str>, apps: &[String]) {
        let (h, r) = (bar_height(), layout.radius);
        let _ = surface.draw(gpu, layout.rect, |dc| unsafe {
            let factory = &gpu.factory;
            // The strip: a full-width rounded pill behind everything.
            if let Ok(brush) = dc.CreateSolidColorBrush(&color(Role::Bg), None) {
                let strip = D2D1_ROUNDED_RECT { rect: rect_f(0.0, 0.0, layout.rect.w, h), radiusX: (h / 2.0) as f32, radiusY: (h / 2.0) as f32 };
                dc.FillRoundedRectangle(&strip, &brush);
            }
            let border = dc.CreateSolidColorBrush(&color(Role::Border), None).ok();

            if let Some(lead) = &layout.lead {
                if let (Ok(path), Some(border)) = (paths::lead(factory, lead.width, r, h), border.as_ref()) {
                    if let Ok(fill) = dc.CreateSolidColorBrush(&color(Role::Bg), None) {
                        dc.FillGeometry(&path, &fill, None);
                    }
                    dc.DrawGeometry(&path, border, 1.0, None);
                }
                if let Ok(accent) = dc.CreateSolidColorBrush(&color(Role::Accent), None) {
                    match &lead.glyph {
                        LeadGlyph::Logo(area) => {
                            if let Ok(mark) = paths::vase_mark(factory, *area, h) {
                                dc.FillGeometry(&mark, &accent, None);
                            }
                        }
                        LeadGlyph::Glyph { x, text, size } => {
                            draw_text(gpu, dc, text, *size, *x, &accent);
                        }
                    }
                }
            }

            for tab in &layout.tabs {
                let Ok(path) = paths::tab(factory, tab.x0, tab.x1, tab.cap_left, r, h) else { continue };
                if let Ok(fill) = dc.CreateSolidColorBrush(&color(tab.fill), None) {
                    dc.FillGeometry(&path, &fill, None);
                }
                if let Some(border) = border.as_ref() {
                    dc.DrawGeometry(&path, border, 1.0, None);
                }
                for run in &tab.content {
                    match run {
                        Run::Text { x, text, color: role } => {
                            if let Ok(brush) = dc.CreateSolidColorBrush(&color(*role), None) {
                                draw_text(gpu, dc, text, FONT_SIZE, *x, &brush);
                            }
                        }
                        Run::Icon { x, app, dim, badge } => {
                            let y = (h - TAB_ICON) / 2.0;
                            if let Some(bitmap) = icons.get(app) {
                                let dest = rect_f(*x, y, TAB_ICON, TAB_ICON);
                                dc.DrawBitmap(bitmap, Some(&dest), if *dim { 0.4 } else { 1.0 }, D2D1_INTERPOLATION_MODE_LINEAR, None, None);
                            }
                            if *badge {
                                if let Ok(brush) = dc.CreateSolidColorBrush(&color(Role::Badge), None) {
                                    let d = 6.0;
                                    dc.FillEllipse(&ellipse(x + TAB_ICON - d / 2.0, y + d / 2.0, d / 2.0), &brush);
                                }
                            }
                        }
                    }
                }
            }
            // Hotkey outlines last, so no neighbour's fill covers a convex-right side.
            if let Ok(bright) = dc.CreateSolidColorBrush(&color(Role::Hotkey), None) {
                for tab in layout.tabs.iter().filter(|t| t.hotkey) {
                    if let Ok(path) = paths::tab(factory, tab.x0, tab.x1, tab.cap_left, r, h) {
                        dc.DrawGeometry(&path, &bright, 1.5, None);
                    }
                }
            }
            // The trailing windowless-app icons, between the last tab and the dot.
            for (x, app) in layout.apps.iter().zip(apps) {
                if let Some(bitmap) = icons.get(app) {
                    let y = (h - TAB_ICON) / 2.0;
                    dc.DrawBitmap(bitmap, Some(&rect_f(*x, y, TAB_ICON, TAB_ICON)), 1.0, D2D1_INTERPOLATION_MODE_LINEAR, None, None);
                }
            }
            if let Some((dot_x, armed)) = layout.dot {
                let role = if armed { Role::Accent } else { Role::Dim };
                if let Ok(brush) = dc.CreateSolidColorBrush(&color(role), None) {
                    dc.FillEllipse(&ellipse(dot_x + DOT_D / 2.0, h / 2.0, DOT_D / 2.0), &brush);
                }
            }
            if let Some(text) = prompt {
                if let Ok(brush) = dc.CreateSolidColorBrush(&color(Role::Text), None) {
                    draw_text(gpu, dc, text, FONT_SIZE, layout.prompt_x(), &brush);
                }
            }
        });
    }
}

impl Painter for D2DPainter {
    fn measure(&self, text: &str, size: f64) -> f64 {
        self.gpu.measure(text, size)
    }

    fn bar(&mut self, bar: &Bar) -> BarHits {
        self.icons.collect(&self.gpu);
        let laid = self.lay_out(bar);
        Self::paint_bar(&self.gpu, &self.icons, &mut self.bar, &laid, None, bar.apps);
        self.gpu.commit();
        laid.hits()
    }

    fn prompt(&mut self, rect: Rect, text: &str) {
        // The command line owns the bar: the mark stays, the tabs do not.
        let bare = self.lay_out(&Bar { rect, tabs: &[], apps: &[], selected: 0, main: true, armed: false }).bare();
        Self::paint_bar(&self.gpu, &self.icons, &mut self.bar, &bare, Some(text), &[]);
        self.gpu.commit();
    }

    fn hide_bar(&mut self) {
        self.bar.hide();
    }

    fn stack_bars(&mut self, bars: &[Bar]) -> Vec<Hits> {
        // A stack bar carries no trailing icons, so only its tab spans can be clicked.
        self.icons.collect(&self.gpu);
        while self.stack_bars.len() < bars.len() {
            let Ok(surface) = Surface::new(&self.gpu) else { break };
            self.stack_bars.push(surface);
        }
        let laid: Vec<Laid> = bars.iter().map(|bar| self.lay_out(bar)).collect();
        for (surface, bar) in self.stack_bars.iter_mut().zip(&laid) {
            Self::paint_bar(&self.gpu, &self.icons, surface, bar, None, &[]);
        }
        for surface in &mut self.stack_bars[bars.len()..] {
            surface.hide();
        }
        self.gpu.commit();
        laid.iter().map(|bar| bar.hits().tabs).collect()
    }

    fn panes(&mut self, panes: &[(Rect, bool)]) {
        if panes.is_empty() {
            self.panes.hide();
            return;
        }
        // One surface covering every placeholder, so they share a single composition window.
        let area = bbox(&panes.iter().map(|(r, _)| *r).collect::<Vec<_>>());
        let _ = self.panes.draw(&self.gpu, area, |dc| unsafe {
            for (rect, focused) in panes {
                let local = rect_f(rect.x - area.x, rect.y - area.y, rect.w, rect.h);
                let rounded = D2D1_ROUNDED_RECT { rect: local, radiusX: PANE_RADIUS as f32, radiusY: PANE_RADIUS as f32 };
                let fill = if *focused { Role::Active } else { Role::Bg };
                if let Ok(brush) = dc.CreateSolidColorBrush(&color(fill), None) {
                    dc.FillRoundedRectangle(&rounded, &brush);
                }
                if *focused {
                    if let Ok(brush) = dc.CreateSolidColorBrush(&color(Role::Accent), None) {
                        dc.DrawRoundedRectangle(&rounded, &brush, 2.0, None);
                    }
                }
            }
        });
        self.gpu.commit();
    }

    fn focus_border(&mut self, rect: Option<Rect>) {
        let Some(rect) = rect else {
            self.focus.hide();
            return;
        };
        // The band has to hold the whole outline, whose rounded corners bow inwards by the radius.
        let _ = self.focus.draw_outline(&self.gpu, rect, PANE_RADIUS + 2.0, |dc| unsafe {
            if let Ok(brush) = dc.CreateSolidColorBrush(&color(Role::Accent), None) {
                // Inset by half the stroke so the 2px outline is not clipped at the surface edge.
                let rounded = D2D1_ROUNDED_RECT { rect: rect_f(1.0, 1.0, rect.w - 2.0, rect.h - 2.0), radiusX: PANE_RADIUS as f32, radiusY: PANE_RADIUS as f32 };
                dc.DrawRoundedRectangle(&rounded, &brush, 2.0, None);
            }
        });
        self.gpu.commit();
    }

    fn list(&mut self, at: ListAt, header: &str, rows: &[SwitchRow], selected: usize) {
        self.icons.collect(&self.gpu);
        let native = matches!(style(), Style::Native);
        let (area, visible, border_role, border_w) = match at {
            ListAt::Centered(screen) => {
                // Cap by screen height (less the header row), not a fixed count, so a taller screen shows more.
                let fit = ((screen.h * LIST_MAX_SCREEN_FRAC - 2.0 * PANE_PAD) / ROW_H) as usize;
                let shown = rows.len().min(fit.saturating_sub(1).max(1));
                let h = (shown + 1) as f64 * ROW_H + 2.0 * PANE_PAD;
                (Rect::new(screen.x + (screen.w - LIST_WIDTH) / 2.0, screen.y + (screen.h - h) / 2.0, LIST_WIDTH, h), shown, Role::Border, 1.0)
            }
            // Heavier accent border, so an empty pane reads as a container rather than a void.
            ListAt::Filling(area) => {
                let fit = (((area.h - 2.0 * PANE_PAD) / ROW_H).floor() as usize).max(1);
                (area, fit.saturating_sub(1).max(1), Role::Accent, 2.0)
            }
        };
        let offset = vase_core::chrome::scroll_offset(selected, visible);
        let gpu = &self.gpu;
        let icons = &self.icons;
        let _ = self.list.draw(gpu, area, |dc| unsafe {
            let card = D2D1_ROUNDED_RECT { rect: rect_f(0.0, 0.0, area.w, area.h), radiusX: PANE_RADIUS as f32, radiusY: PANE_RADIUS as f32 };
            if let Ok(brush) = dc.CreateSolidColorBrush(&color(Role::Bg), None) {
                dc.FillRoundedRectangle(&card, &brush);
            }
            if let Ok(brush) = dc.CreateSolidColorBrush(&color(border_role), None) {
                dc.DrawRoundedRectangle(&card, &brush, border_w as f32, None);
            }
            // Every run is capped at the card's right padding, so nothing spills past the edge.
            let room = |x: f64| area.w - x - PANE_PAD;
            if let Ok(brush) = dc.CreateSolidColorBrush(&color(Role::Dim), None) {
                draw_row_text(gpu, dc, header, PANE_PAD, PANE_PAD, room(PANE_PAD), &brush);
            }
            for slot in 0..visible {
                let Some(row) = rows.get(offset + slot) else { break };
                let y = PANE_PAD + ((slot + 1) as f64) * ROW_H;
                if offset + slot == selected {
                    if let Ok(brush) = dc.CreateSolidColorBrush(&color(Role::Active), None) {
                        // Fluent insets a selected row and rounds it; a painted card highlights to its inner edge.
                        if native {
                            dc.FillRoundedRectangle(&rounded_row(PANE_PAD / 2.0, y, area.w - PANE_PAD, ROW_H), &brush);
                        } else {
                            dc.FillRectangle(&rect_f(1.0, y, area.w - 2.0, ROW_H), &brush);
                        }
                    }
                }
                // The left accent marks the focused window, so it stays marked as the selection moves away.
                if row.current {
                    if let Ok(brush) = dc.CreateSolidColorBrush(&color(Role::Accent), None) {
                        dc.FillRectangle(&rect_f(1.0, y, 3.0, ROW_H), &brush);
                    }
                }
                let mut x = PANE_PAD;
                let marker = bar::row_marker(row.favorite, row.off_workspace);
                if let Ok(brush) = dc.CreateSolidColorBrush(&color(Role::Accent), None) {
                    draw_row_text(gpu, dc, marker, x, y, room(x), &brush);
                }
                x += gpu.measure(marker, FONT_SIZE) + 4.0;
                if row.number > 0 {
                    let n = format!("{:>2} ", row.number);
                    if let Ok(brush) = dc.CreateSolidColorBrush(&color(Role::Dim), None) {
                        draw_row_text(gpu, dc, &n, x, y, room(x), &brush);
                    }
                    x += gpu.measure(&n, FONT_SIZE);
                }
                if !row.prefix.is_empty() {
                    if let Ok(brush) = dc.CreateSolidColorBrush(&color(Role::Dim), None) {
                        draw_row_text(gpu, dc, &row.prefix, x, y, room(x), &brush);
                    }
                    x += gpu.measure(&row.prefix, FONT_SIZE);
                }
                for app in &row.icons {
                    if let Some(bitmap) = icons.get(app) {
                        let iy = y + (ROW_H - 16.0) / 2.0;
                        dc.DrawBitmap(bitmap, Some(&rect_f(x, iy, 16.0, 16.0)), if row.dim { 0.45 } else { 1.0 }, D2D1_INTERPOLATION_MODE_LINEAR, None, None);
                    }
                    x += 20.0;
                }
                if let Ok(brush) = dc.CreateSolidColorBrush(&color(Role::Text), None) {
                    draw_row_text(gpu, dc, &row.label, x, y, room(x), &brush);
                }
            }
        });
        self.gpu.commit();
    }

    fn hide_list(&mut self) {
        self.list.hide();
    }

    fn hide_bars(&mut self) {
        self.bar.hide();
        for surface in &mut self.stack_bars {
            surface.hide();
        }
    }

    fn hide_all(&mut self) {
        self.hide_bars();
        self.panes.hide();
        self.focus.hide();
        self.list.hide();
    }

    fn prewarm_icon(&mut self, app: &str) {
        self.icons.warm(app);
    }
}

fn rect_f(x: f64, y: f64, w: f64, h: f64) -> D2D_RECT_F {
    D2D_RECT_F { left: x as f32, top: y as f32, right: (x + w) as f32, bottom: (y + h) as f32 }
}

/// A list row's highlight, rounded to a Fluent corner.
fn rounded_row(x: f64, y: f64, w: f64, h: f64) -> D2D1_ROUNDED_RECT {
    D2D1_ROUNDED_RECT { rect: rect_f(x, y, w, h), radiusX: fluent::CORNER as f32, radiusY: fluent::CORNER as f32 }
}

fn ellipse(cx: f64, cy: f64, r: f64) -> D2D1_ELLIPSE {
    D2D1_ELLIPSE { point: Vector2 { X: cx as f32, Y: cy as f32 }, radiusX: r as f32, radiusY: r as f32 }
}

/// Draw a bar run at `x`, vertically centred in the strip.
fn draw_text(gpu: &Gpu, dc: &ID2D1DeviceContext, text: &str, size: f64, x: f64, brush: &ID2D1Brush) {
    let Ok(layout) = gpu.layout(text, size) else { return };
    let mut m = DWRITE_TEXT_METRICS::default();
    let _ = unsafe { layout.GetMetrics(&mut m) };
    let y = (bar_height() - m.height as f64) / 2.0;
    unsafe { dc.DrawTextLayout(Vector2 { X: x as f32, Y: y as f32 }, &layout, brush, D2D1_DRAW_TEXT_OPTIONS_NONE) };
}

/// Draw a list-row run at `x`, vertically centred in its row and ellipsized at `max`.
fn draw_row_text(gpu: &Gpu, dc: &ID2D1DeviceContext, text: &str, x: f64, row_y: f64, max: f64, brush: &ID2D1Brush) {
    let Ok(layout) = gpu.trimmed(text, FONT_SIZE, max) else { return };
    let mut m = DWRITE_TEXT_METRICS::default();
    let _ = unsafe { layout.GetMetrics(&mut m) };
    let y = row_y + (ROW_H - m.height as f64) / 2.0;
    unsafe { dc.DrawTextLayout(Vector2 { X: x as f32, Y: y as f32 }, &layout, brush, D2D1_DRAW_TEXT_OPTIONS_NONE) };
}
