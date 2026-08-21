use super::*;
use vase_core::chrome::bar::BarTab;
use vase_core::chrome::theme::{set_theme, Style, Theme};

/// A fixed-pitch stand-in for Segoe's metrics.
fn measure(text: &str, size: f64) -> f64 {
    text.chars().count() as f64 * size * 0.6
}

fn tab(number: usize) -> BarTab {
    BarTab { icons: vec!["Ghostty".into()], badges: vec![false], label: "window".into(), zoomed: false, number, dim: false, off_workspace: false, hotkey: false }
}

fn strip(tabs: &[BarTab], main: bool) -> Strip {
    set_theme(Theme { style: Style::Native, ..Theme::DEFAULT });
    layout(&Bar { rect: Rect::new(0.0, 0.0, 1200.0, bar_height()), tabs, selected: 1, main, armed: false }, &measure)
}

#[test]
fn tabs_are_separated_by_a_gap_whose_halves_stay_clickable() {
    let strip = strip(&[tab(1), tab(2), tab(3)], true);
    for pair in strip.tabs.windows(2) {
        assert_eq!(pair[1].x0 - (pair[0].x0 + pair[0].w), TAB_GAP, "tabs sit apart on the strip, they do not interlock");
    }
    // Content sits inside its own tab, and adjacent hit ranges meet exactly.
    for (tab, (a, b)) in strip.tabs.iter().zip(strip.hits()) {
        let xs: Vec<f64> = tab
            .runs
            .iter()
            .map(|r| match r {
                Run::Text { x, .. } | Run::Icon { x, .. } => *x,
            })
            .collect();
        assert!(xs.iter().all(|x| *x > tab.x0 && *x < tab.x0 + tab.w), "{xs:?} spills out of its tab");
        assert!(a <= tab.x0 && b >= tab.x0 + tab.w);
    }
    assert!(strip.hits().windows(2).all(|w| w[0].1 == w[1].0), "no click may fall between two tabs");
}

#[test]
fn only_the_selected_tab_carries_the_accent_fill() {
    let strip = strip(&[tab(1), tab(2)], true);
    assert_eq!(strip.tabs.iter().map(|t| t.fill).collect::<Vec<_>>(), vec![None, Some(Role::Accent)]);
    assert_eq!(strip.tabs.iter().filter(|t| t.selected).count(), 1);
}

#[test]
fn a_stack_bar_carries_no_mark_and_no_prefix_dot() {
    let stack = strip(&[tab(1)], false);
    assert!(stack.slot.is_none() && stack.dot.is_none());
    assert_eq!(stack.tabs[0].x0, PAD, "its first tab starts at the strip's own padding");
    assert!(strip(&[tab(1)], true).tabs[0].x0 > PAD, "where the screen's bar clears the mark first");
}
