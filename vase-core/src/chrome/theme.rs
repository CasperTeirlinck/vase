//! The themeable palette and tab-bar mark.

use std::cell::RefCell;
use std::sync::LazyLock;

use crate::geometry::Rect;

/// Corner radius of a pane placeholder or a list card.
pub const PANE_RADIUS: f64 = 8.0;
/// Inset between a pane's edge and its content.
pub const PANE_PAD: f64 = 8.0;

/// A semantic palette slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Tab-bar strip and inactive tabs.
    Bg,
    /// Selected tab and list highlight.
    Active,
    /// Recessed fill for an off-monitor tab.
    DimBg,
    /// Primary text.
    Text,
    /// Secondary text: index numbers, dimmed rows.
    Dim,
    /// The mark, focus outline, current-row accent, markers.
    Accent,
    /// Notification dot.
    Badge,
    /// Tab and card outlines.
    Border,
    /// Bright outline for a focus-hotkey tab.
    Hotkey,
}

/// A full color palette; each field is sRGBA in 0..1.
#[derive(Clone, Copy)]
pub struct Theme {
    pub bg: [f64; 4],
    pub active: [f64; 4],
    pub dim_bg: [f64; 4],
    pub text: [f64; 4],
    pub dim: [f64; 4],
    pub accent: [f64; 4],
    pub badge: [f64; 4],
    pub border: [f64; 4],
    pub hotkey: [f64; 4],
}

impl Theme {
    pub fn color(&self, role: Role) -> [f64; 4] {
        match role {
            Role::Bg => self.bg,
            Role::Active => self.active,
            Role::DimBg => self.dim_bg,
            Role::Text => self.text,
            Role::Dim => self.dim,
            Role::Accent => self.accent,
            Role::Badge => self.badge,
            Role::Border => self.border,
            Role::Hotkey => self.hotkey,
        }
    }
}

/// The leading element in the main tab bar's left corner.
#[derive(Clone, PartialEq, Eq)]
pub enum Mark {
    /// The built-in vase logo silhouette.
    Logo,
    /// A user glyph.
    Glyph(String),
    /// No leading element; the first tab caps like a stack bar.
    Hidden,
}

// Atom One Dark backgrounds/text with the vase clay accent: the default.
pub const ONE_DARK: Theme = Theme {
    bg: [0.129, 0.145, 0.169, 1.0],
    active: [0.231, 0.247, 0.298, 1.0],
    dim_bg: [0.09, 0.10, 0.125, 1.0],
    text: [0.671, 0.698, 0.749, 1.0],
    dim: [0.361, 0.388, 0.439, 1.0],
    accent: [0.780, 0.400, 0.235, 1.0], // clay #c7663c
    badge: [1.0, 0.271, 0.227, 1.0],
    border: [0.361, 0.388, 0.439, 1.0],
    hotkey: [0.85, 0.87, 0.90, 0.95],
};

const NORD: Theme = Theme {
    bg: [0.180, 0.204, 0.251, 1.0],
    active: [0.263, 0.298, 0.369, 1.0],
    dim_bg: [0.153, 0.173, 0.212, 1.0],
    text: [0.847, 0.871, 0.914, 1.0],
    dim: [0.298, 0.337, 0.416, 1.0],
    accent: [0.533, 0.753, 0.816, 1.0], // frost #88c0d0
    badge: [0.749, 0.380, 0.416, 1.0],
    border: [0.298, 0.337, 0.416, 1.0],
    hotkey: [0.925, 0.937, 0.957, 0.95],
};

const GRUVBOX: Theme = Theme {
    bg: [0.157, 0.157, 0.157, 1.0],
    active: [0.235, 0.220, 0.212, 1.0],
    dim_bg: [0.114, 0.125, 0.129, 1.0],
    text: [0.922, 0.859, 0.698, 1.0],
    dim: [0.573, 0.514, 0.455, 1.0],
    accent: [0.996, 0.502, 0.098, 1.0], // orange #fe8019
    badge: [0.984, 0.286, 0.204, 1.0],
    border: [0.314, 0.286, 0.271, 1.0],
    hotkey: [0.984, 0.945, 0.780, 0.95],
};

const TOKYO_NIGHT: Theme = Theme {
    bg: [0.102, 0.106, 0.149, 1.0],
    active: [0.161, 0.180, 0.259, 1.0],
    dim_bg: [0.086, 0.086, 0.118, 1.0],
    text: [0.753, 0.792, 0.961, 1.0],
    dim: [0.337, 0.373, 0.537, 1.0],
    accent: [0.478, 0.635, 0.969, 1.0], // blue #7aa2f7
    badge: [0.969, 0.463, 0.557, 1.0],
    border: [0.231, 0.259, 0.380, 1.0],
    hotkey: [0.753, 0.792, 0.961, 0.95],
};

const CATPPUCCIN: Theme = Theme {
    bg: [0.118, 0.118, 0.180, 1.0],
    active: [0.192, 0.196, 0.267, 1.0],
    dim_bg: [0.094, 0.094, 0.145, 1.0],
    text: [0.804, 0.839, 0.957, 1.0],
    dim: [0.424, 0.439, 0.525, 1.0],
    accent: [0.980, 0.702, 0.529, 1.0], // peach #fab387
    badge: [0.953, 0.545, 0.659, 1.0],
    border: [0.271, 0.278, 0.353, 1.0],
    hotkey: [0.804, 0.839, 0.957, 0.95],
};

/// Windows' Fluent palette, so the chrome can read as native there. Greys follow the Fluent neutral ramp.
const FLUENT_DARK: Theme = Theme {
    bg: [0.129, 0.129, 0.129, 1.0],
    active: [0.216, 0.216, 0.216, 1.0],
    dim_bg: [0.102, 0.102, 0.102, 1.0],
    text: [1.000, 1.000, 1.000, 1.0],
    dim: [0.612, 0.612, 0.612, 1.0],
    accent: [0.376, 0.694, 0.910, 1.0],
    badge: [1.000, 0.600, 0.600, 1.0],
    border: [0.267, 0.267, 0.267, 1.0],
    hotkey: [0.900, 0.900, 0.900, 0.95],
};

/// The only light palette vase ships.
const FLUENT_LIGHT: Theme = Theme {
    bg: [0.973, 0.973, 0.973, 1.0],     // SolidBackgroundFillColorBase
    active: [0.902, 0.902, 0.902, 1.0], // ControlFillColorSecondary
    dim_bg: [0.937, 0.937, 0.937, 1.0],
    text: [0.100, 0.100, 0.100, 1.0], // TextFillColorPrimary
    dim: [0.400, 0.400, 0.400, 1.0],  // TextFillColorSecondary
    accent: [0.000, 0.475, 0.843, 1.0],
    badge: [0.769, 0.169, 0.110, 1.0], // SystemFillColorCritical
    border: [0.851, 0.851, 0.851, 1.0],
    hotkey: [0.200, 0.200, 0.200, 0.95],
};

/// A built-in theme by config name.
pub fn by_name(name: &str) -> Option<Theme> {
    match name.trim().to_lowercase().replace([' ', '_'], "-").as_str() {
        "one-dark" | "onedark" => Some(ONE_DARK),
        "nord" => Some(NORD),
        "gruvbox" | "gruvbox-dark" => Some(GRUVBOX),
        "tokyo-night" | "tokyonight" => Some(TOKYO_NIGHT),
        "catppuccin" | "catppuccin-mocha" => Some(CATPPUCCIN),
        "fluent" | "fluent-dark" => Some(FLUENT_DARK),
        "fluent-light" => Some(FLUENT_LIGHT),
        _ => None,
    }
}

/// Every built-in name, for the docs and for tests that must cover the whole set.
pub const PRESETS: [&str; 7] = ["one-dark", "nord", "gruvbox", "tokyo-night", "catppuccin", "fluent-dark", "fluent-light"];

/// Parse `#rgb`, `#rrggbb`, or `#rrggbbaa` into sRGBA in 0..1.
pub fn parse_hex(s: &str) -> Option<[f64; 4]> {
    let h = s.trim().strip_prefix('#')?;
    let byte = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok().map(|v| v as f64 / 255.0);
    match h.len() {
        3 => {
            let d = |i: usize| u8::from_str_radix(&h[i..i + 1], 16).ok().map(|v| (v * 17) as f64 / 255.0);
            Some([d(0)?, d(1)?, d(2)?, 1.0])
        }
        6 => Some([byte(0)?, byte(2)?, byte(4)?, 1.0]),
        8 => Some([byte(0)?, byte(2)?, byte(4)?, byte(6)?]),
        _ => None,
    }
}

// Set at startup and on a config reload; every painter reads this same pair.
thread_local! {
    static CURRENT: RefCell<Theme> = const { RefCell::new(ONE_DARK) };
    static MARK: RefCell<Mark> = const { RefCell::new(Mark::Logo) };
}

pub fn set_theme(theme: Theme) {
    CURRENT.with(|c| *c.borrow_mut() = theme);
}

pub fn theme() -> Theme {
    CURRENT.with(|c| *c.borrow())
}

pub fn set_mark(mark: Mark) {
    MARK.with(|m| *m.borrow_mut() = mark);
}

pub fn mark() -> Mark {
    MARK.with(|m| m.borrow().clone())
}

/// The brand mark's silhouette polygon, normalized to a 0..1 box with `y` running top→bottom.
pub struct VaseMark {
    pub points: Vec<(f64, f64)>,
    pub aspect: f64,
}

impl VaseMark {
    /// The points scaled into `area`, `y` flipped so the vase stands up in bottom-left origin.
    pub fn polygon(&self, area: Rect) -> Vec<(f64, f64)> {
        self.points.iter().map(|(nx, ny)| (area.x + nx * area.w, area.y + (1.0 - ny) * area.h)).collect()
    }
}

/// The brand mark, parsed from the SVG so the drawn logo can't drift from the file.
pub fn vase_mark() -> &'static VaseMark {
    &VASE_MARK
}

static VASE_MARK: LazyLock<VaseMark> = LazyLock::new(|| {
    let svg = include_str!("../../../docs/branding/vase-mark.svg");
    let attr = |name: &str| svg.split_once(&format!("{name}=\"")).unwrap().1.split_once('"').unwrap().0;
    let view_box: Vec<f64> = attr("viewBox").split_whitespace().map(|n| n.parse().unwrap()).collect();
    let (w, h) = (view_box[2], view_box[3]);
    let points = attr("points")
        .split_whitespace()
        .map(|pt| {
            let (px, py) = pt.split_once(',').unwrap();
            (px.parse::<f64>().unwrap() / w, py.parse::<f64>().unwrap() / h)
        })
        .collect();
    VaseMark { points, aspect: w / h }
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_parses_all_forms() {
        assert_eq!(parse_hex("#fff"), Some([1.0, 1.0, 1.0, 1.0]));
        assert_eq!(parse_hex("#000000"), Some([0.0, 0.0, 0.0, 1.0]));
        assert_eq!(parse_hex("#ff0000ff"), Some([1.0, 0.0, 0.0, 1.0]));
        assert!(parse_hex("nope").is_none());
        assert!(parse_hex("#12345").is_none());
    }

    #[test]
    fn named_themes_resolve() {
        assert!(by_name("nord").is_some());
        assert!(by_name("Tokyo Night").is_some());
        assert!(by_name("bogus").is_none());
    }

    #[test]
    fn vase_mark_svg_parses() {
        let mark = vase_mark();
        assert!(mark.points.len() > 10);
        assert!((mark.aspect - 677.0 / 744.0).abs() < 1e-9);
        assert!(mark.points.iter().all(|&(x, y)| (0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y)));
    }
}
