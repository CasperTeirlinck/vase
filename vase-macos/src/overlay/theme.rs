//! The themeable palette and tab-bar mark, shared across the overlay windows.

use std::cell::RefCell;

use objc2::rc::Retained;
use objc2_app_kit::NSColor;

pub(crate) const PANE_RADIUS: f64 = 8.0;
pub(crate) const PANE_PAD: f64 = 8.0;

/// A full color palette; each field is sRGBA in 0..1.
#[derive(Clone, Copy)]
pub(crate) struct Theme {
    pub bg: [f64; 4],     // tab-bar strip and inactive tabs
    pub active: [f64; 4], // selected tab and list highlight
    pub dim_bg: [f64; 4], // recessed fill for an off-monitor tab
    pub text: [f64; 4],   // primary text
    pub dim: [f64; 4],    // secondary text (index numbers, dimmed rows)
    pub accent: [f64; 4], // the mark, focus outline, current-row accent, markers
    pub badge: [f64; 4],  // notification dot
    pub border: [f64; 4], // tab and card outlines
    pub hotkey: [f64; 4], // bright outline for a focus-hotkey tab
}

/// The leading element in the main tab bar's left corner.
#[derive(Clone)]
pub(crate) enum Mark {
    /// The built-in vase logo silhouette.
    Logo,
    /// A user glyph rendered as text.
    Glyph(String),
    /// No leading element; the first tab caps like a stack bar.
    Hidden,
}

// Atom One Dark backgrounds/text with the vase clay accent: the default.
pub(crate) const ONE_DARK: Theme = Theme {
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

/// A built-in theme by config name, or `None` for an unknown name.
pub(crate) fn by_name(name: &str) -> Option<Theme> {
    match name.trim().to_lowercase().replace([' ', '_'], "-").as_str() {
        "one-dark" | "onedark" => Some(ONE_DARK),
        "nord" => Some(NORD),
        "gruvbox" | "gruvbox-dark" => Some(GRUVBOX),
        "tokyo-night" | "tokyonight" => Some(TOKYO_NIGHT),
        "catppuccin" | "catppuccin-mocha" => Some(CATPPUCCIN),
        _ => None,
    }
}

/// Parse `#rgb`, `#rrggbb`, or `#rrggbbaa` into sRGBA in 0..1.
pub(crate) fn parse_hex(s: &str) -> Option<[f64; 4]> {
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

thread_local! {
    static CURRENT: RefCell<Theme> = const { RefCell::new(ONE_DARK) };
    static MARK: RefCell<Mark> = const { RefCell::new(Mark::Logo) };
}

pub(crate) fn set_theme(theme: Theme) {
    CURRENT.with(|c| *c.borrow_mut() = theme);
}

pub(crate) fn set_mark(mark: Mark) {
    MARK.with(|m| *m.borrow_mut() = mark);
}

pub(crate) fn mark() -> Mark {
    MARK.with(|m| m.borrow().clone())
}

fn color(c: [f64; 4]) -> Retained<NSColor> {
    NSColor::colorWithSRGBRed_green_blue_alpha(c[0], c[1], c[2], c[3])
}

fn current<T>(f: impl FnOnce(&Theme) -> T) -> T {
    CURRENT.with(|c| f(&c.borrow()))
}

pub(crate) fn strip_bg() -> Retained<NSColor> {
    color(current(|t| t.bg))
}
pub(crate) fn active_bg() -> Retained<NSColor> {
    color(current(|t| t.active))
}
pub(crate) fn dim_bg() -> Retained<NSColor> {
    color(current(|t| t.dim_bg))
}
pub(crate) fn text_col() -> Retained<NSColor> {
    color(current(|t| t.text))
}
pub(crate) fn dim_col() -> Retained<NSColor> {
    color(current(|t| t.dim))
}
// The mark and every highlight (focus outline, current-row accent, off-Space and favorite markers, armed-prefix dot).
pub(crate) fn accent() -> Retained<NSColor> {
    color(current(|t| t.accent))
}
pub(crate) fn badge_red() -> Retained<NSColor> {
    color(current(|t| t.badge))
}
pub(crate) fn tab_border() -> Retained<NSColor> {
    color(current(|t| t.border))
}
pub(crate) fn hotkey_border() -> Retained<NSColor> {
    color(current(|t| t.hotkey))
}
// The focused-pane accent (outline / placeholder border).
pub(crate) fn pane_border() -> Retained<NSColor> {
    accent()
}

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
}
