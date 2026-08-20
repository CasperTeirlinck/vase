//! The Direct2D painter: vase's chrome, drawn in the Fluent idiom.

mod gpu;
mod icons;
pub(crate) mod paths;

use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;
use windows::Win32::Graphics::Direct2D::{ID2D1Brush, ID2D1DeviceContext, D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_ELLIPSE, D2D1_INTERPOLATION_MODE_LINEAR, D2D1_ROUNDED_RECT};
use windows::Win32::Graphics::DirectWrite::DWRITE_TEXT_METRICS;
use windows_numerics::Vector2;

use vase_core::chrome::bar::{BarLayout, Run, DOT_D, TAB_ICON};
use vase_core::chrome::theme::{Role, PANE_PAD, PANE_RADIUS};
use vase_core::chrome::{bar, ListAt, Painter, SwitchRow, BAR_HEIGHT, FONT_SIZE};
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
    /// Pool of local powerline bars, one per visible stack.
    stack_bars: Vec<Surface>,
    panes: Surface,
    focus: Surface,
    list: Surface,
    icons: Icons,
}

impl D2DPainter {
    pub fn new() -> windows::core::Result<D2DPainter> {
        let gpu = Gpu::new()?;
        Ok(D2DPainter { bar: Surface::new(&gpu)?, stack_bars: Vec::new(), panes: Surface::new(&gpu)?, focus: Surface::new(&gpu)?, list: Surface::new(&gpu)?, icons: Icons::default(), gpu })
    }

    /// Paint one laid-out bar into `surface`.
    fn paint_bar(gpu: &Gpu, icons: &Icons, surface: &mut Surface, layout: &BarLayout) {
        let r = layout.radius;
        let _ = surface.draw(gpu, layout.rect, |dc| unsafe {
            let factory = &gpu.factory;
            // The strip: a full-width rounded pill behind everything.
            if let Ok(brush) = dc.CreateSolidColorBrush(&color(Role::Bg), None) {
                let strip = D2D1_ROUNDED_RECT { rect: rect_f(0.0, 0.0, layout.rect.w, BAR_HEIGHT), radiusX: (BAR_HEIGHT / 2.0) as f32, radiusY: (BAR_HEIGHT / 2.0) as f32 };
                dc.FillRoundedRectangle(&strip, &brush);
            }
            let border = dc.CreateSolidColorBrush(&color(Role::Border), None).ok();

            if let Some(lead) = &layout.lead {
                if let (Ok(path), Some(border)) = (paths::lead(factory, lead.width, r, BAR_HEIGHT), border.as_ref()) {
                    if let Ok(fill) = dc.CreateSolidColorBrush(&color(Role::Bg), None) {
                        dc.FillGeometry(&path, &fill, None);
                    }
                    dc.DrawGeometry(&path, border, 1.0, None);
                }
                if let Ok(accent) = dc.CreateSolidColorBrush(&color(Role::Accent), None) {
                    match &lead.glyph {
                        vase_core::chrome::bar::LeadGlyph::Logo(area) => {
                            if let Ok(mark) = paths::vase_mark(factory, *area, BAR_HEIGHT) {
                                dc.FillGeometry(&mark, &accent, None);
                            }
                        }
                        vase_core::chrome::bar::LeadGlyph::Glyph { x, text, size } => {
                            draw_text(gpu, dc, text, *size, *x, &accent);
                        }
                    }
                }
            }

            for tab in &layout.tabs {
                let Ok(path) = paths::tab(factory, tab.x0, tab.x1, tab.cap_left, r, BAR_HEIGHT) else { continue };
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
                            let y = (BAR_HEIGHT - TAB_ICON) / 2.0;
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
                    if let Ok(path) = paths::tab(factory, tab.x0, tab.x1, tab.cap_left, r, BAR_HEIGHT) {
                        dc.DrawGeometry(&path, &bright, 1.5, None);
                    }
                }
            }
            if let Some((dot_x, armed)) = layout.dot {
                let role = if armed { Role::Accent } else { Role::Dim };
                if let Ok(brush) = dc.CreateSolidColorBrush(&color(role), None) {
                    dc.FillEllipse(&ellipse(dot_x + DOT_D / 2.0, BAR_HEIGHT / 2.0, DOT_D / 2.0), &brush);
                }
            }
        });
    }
}

impl Painter for D2DPainter {
    fn measure(&self, text: &str, size: f64) -> f64 {
        self.gpu.measure(text, size)
    }

    fn bar(&mut self, layout: &BarLayout) {
        self.icons.collect(&self.gpu);
        Self::paint_bar(&self.gpu, &self.icons, &mut self.bar, layout);
        self.gpu.commit();
    }

    fn prompt(&mut self, layout: &BarLayout, text: &str) {
        // The command line owns the bar: the mark stays, the tabs do not.
        let bare = BarLayout { tabs: Vec::new(), dot: None, ..clone_shell(layout) };
        Self::paint_bar(&self.gpu, &self.icons, &mut self.bar, &bare);
        let lead_w = bare.lead.as_ref().map_or(bare.radius, |l| l.width);
        let x = lead_w + bare.radius + 5.0;
        let gpu = &self.gpu;
        let _ = self.bar.draw(gpu, bare.rect, |dc| unsafe {
            if let Ok(brush) = dc.CreateSolidColorBrush(&color(Role::Text), None) {
                draw_text(gpu, dc, text, FONT_SIZE, x, &brush);
            }
        });
        self.gpu.commit();
    }

    fn hide_bar(&mut self) {
        self.bar.hide();
    }

    fn stack_bars(&mut self, layouts: &[BarLayout]) {
        self.icons.collect(&self.gpu);
        while self.stack_bars.len() < layouts.len() {
            let Ok(surface) = Surface::new(&self.gpu) else { break };
            self.stack_bars.push(surface);
        }
        for (surface, layout) in self.stack_bars.iter_mut().zip(layouts) {
            Self::paint_bar(&self.gpu, &self.icons, surface, layout);
        }
        for surface in &mut self.stack_bars[layouts.len()..] {
            surface.hide();
        }
        self.gpu.commit();
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
                        dc.FillRectangle(&rect_f(1.0, y, area.w - 2.0, ROW_H), &brush);
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

/// Everything but the tabs and the dot, for the command line to reuse.
fn clone_shell(layout: &BarLayout) -> BarLayout {
    BarLayout { rect: layout.rect, lead: layout.lead.clone(), tabs: Vec::new(), dot: None, radius: layout.radius, content_w: layout.content_w }
}

fn rect_f(x: f64, y: f64, w: f64, h: f64) -> D2D_RECT_F {
    D2D_RECT_F { left: x as f32, top: y as f32, right: (x + w) as f32, bottom: (y + h) as f32 }
}

fn ellipse(cx: f64, cy: f64, r: f64) -> D2D1_ELLIPSE {
    D2D1_ELLIPSE { point: Vector2 { X: cx as f32, Y: cy as f32 }, radiusX: r as f32, radiusY: r as f32 }
}

/// Draw a bar run at `x`, vertically centred in the strip.
fn draw_text(gpu: &Gpu, dc: &ID2D1DeviceContext, text: &str, size: f64, x: f64, brush: &ID2D1Brush) {
    let Ok(layout) = gpu.layout(text, size) else { return };
    let mut m = DWRITE_TEXT_METRICS::default();
    let _ = unsafe { layout.GetMetrics(&mut m) };
    let y = (BAR_HEIGHT - m.height as f64) / 2.0;
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
