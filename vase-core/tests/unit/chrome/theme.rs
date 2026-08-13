use crate::chrome::theme::{by_name, PRESETS};

/// Relative luminance (WCAG).
fn luminance(c: [f64; 4]) -> f64 {
    let channel = |v: f64| if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
    0.2126 * channel(c[0]) + 0.7152 * channel(c[1]) + 0.0722 * channel(c[2])
}

fn contrast(a: [f64; 4], b: [f64; 4]) -> f64 {
    let (x, y) = (luminance(a), luminance(b));
    let (hi, lo) = if x > y { (x, y) } else { (y, x) };
    (hi + 0.05) / (lo + 0.05)
}

#[test]
fn every_preset_keeps_text_readable_on_its_own_strip() {
    // 4.5:1 is the WCAG AA threshold for body text; the bar labels sit right at body size.
    for name in PRESETS {
        let theme = by_name(name).unwrap_or_else(|| panic!("{name} is listed but does not resolve"));
        assert!(contrast(theme.text, theme.bg) >= 4.5, "{name}: primary text on the strip");
        assert!(contrast(theme.text, theme.active) >= 4.5, "{name}: primary text on the selected tab");
        assert!(contrast(theme.accent, theme.bg) >= 3.0, "{name}: the mark has to stay visible");
    }
}
