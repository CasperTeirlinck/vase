use crate::chrome::bar::{Bar, BarTab, Run};
use crate::chrome::powerline::*;
use crate::chrome::theme::{set_theme, Mark, Role, Style, Theme};
use crate::geometry::Rect;

/// The strip height these proportions are laid out against.
const H: f64 = Style::Powerline.bar_height();

/// A fixed-pitch stand-in for a painter's text metrics.
fn measure(text: &str, size: f64) -> f64 {
    text.chars().count() as f64 * size * 0.6
}

fn tab(number: usize, label: &str) -> BarTab {
    BarTab { icons: vec!["Ghostty".into()], badges: vec![false], label: label.into(), zoomed: false, number, dim: false, off_workspace: false, hotkey: false }
}

fn strip() -> Rect {
    Rect::new(0.0, 780.0, 1000.0, H)
}

fn lay(tabs: &[BarTab], selected: usize, main: bool, mark: &Mark) -> BarLayout {
    lay_with(tabs, selected, main, mark, &[])
}

fn lay_with(tabs: &[BarTab], selected: usize, main: bool, mark: &Mark, apps: &[String]) -> BarLayout {
    // Lay out against the powerline style's own strip, whatever the default theme is.
    set_theme(Theme { style: Style::Powerline, ..Theme::DEFAULT });
    layout(&Bar { rect: strip(), tabs, apps, selected, main, armed: false }, mark, &measure)
}

#[test]
fn consecutive_tabs_share_an_arc_center_so_the_bulge_nests_the_notch() {
    let tabs = [tab(1, "one"), tab(2, "two"), tab(3, "three")];
    let l = lay(&tabs, 0, true, &Mark::Logo);
    for pair in l.tabs.windows(2) {
        assert_eq!(pair[0].x1, pair[1].x0, "a tab's right center must be the next tab's left center");
    }
}

#[test]
fn the_first_tab_starts_at_the_lead_pill_and_caps_when_there_is_none() {
    let tabs = [tab(1, "one")];

    let with_logo = lay(&tabs, 0, true, &Mark::Logo);
    let lead = with_logo.lead.expect("the main bar shows a mark");
    assert_eq!(with_logo.tabs[0].x0, lead.width);
    assert!(!with_logo.tabs[0].cap_left, "a tab nesting into the pill uses a notch");

    // A hidden mark and a stack bar both drop the pill, so the first tab caps at the strip's corner.
    for l in [lay(&tabs, 0, true, &Mark::Hidden), lay(&tabs, 0, false, &Mark::Logo)] {
        assert!(l.lead.is_none());
        assert!(l.tabs[0].cap_left);
        assert_eq!(l.tabs[0].x0, H / 2.0);
    }
}

#[test]
fn only_the_main_bar_carries_a_prefix_dot() {
    let tabs = [tab(1, "one")];
    assert!(lay(&tabs, 0, true, &Mark::Logo).dot.is_some());
    assert!(lay(&tabs, 0, false, &Mark::Logo).dot.is_none());
    // A stack bar has no dot to avoid, so its content runs the full width.
    assert_eq!(lay(&tabs, 0, false, &Mark::Logo).content_w, strip().w);
}

#[test]
fn hit_ranges_sit_a_radius_right_of_the_logical_span() {
    let tabs = [tab(1, "one"), tab(2, "two")];
    let l = lay(&tabs, 0, true, &Mark::Logo);
    let ranges = l.hits();
    assert_eq!(ranges.len(), 2);
    for (shape, (a, b)) in l.tabs.iter().zip(&ranges) {
        assert_eq!(*a, shape.x0 + l.radius);
        assert_eq!(*b, shape.x1 + l.radius);
    }
    // Adjacent ranges meet exactly, so no click lands between two tabs.
    assert_eq!(ranges[0].1, ranges[1].0);
}

#[test]
fn the_selected_tab_fills_active_and_an_off_monitor_tab_stays_recessed() {
    let mut tabs = [tab(1, "one"), tab(2, "two")];
    let l = lay(&tabs, 1, true, &Mark::Logo);
    assert_eq!(l.tabs[0].fill, Role::Bg);
    assert_eq!(l.tabs[1].fill, Role::Active);

    // Dim wins over selected: a tab on another monitor never reads as the active one.
    tabs[1].dim = true;
    assert_eq!(lay(&tabs, 1, true, &Mark::Logo).tabs[1].fill, Role::DimBg);
}

#[test]
fn a_long_label_ellipsizes_to_fit_and_a_short_one_is_untouched() {
    let long = "a very long window title that will not fit inside one tab at all";
    let l = lay(&[tab(1, long)], 0, true, &Mark::Logo);
    let Some(Run::Text { text, .. }) = l.tabs[0].content.last() else { panic!("the label is the last run") };
    assert!(text.ends_with('…'));
    assert!(measure(text, crate::chrome::FONT_SIZE) <= 140.0, "the ellipsized label must fit the cap");
    assert!(text.chars().count() > 5, "it must keep as much of the title as fits");

    let l = lay(&[tab(1, "short")], 0, true, &Mark::Logo);
    let Some(Run::Text { text, .. }) = l.tabs[0].content.last() else { panic!("the label is the last run") };
    assert_eq!(text, "short");
}

#[test]
fn content_runs_are_ordered_marker_then_number_then_icons_then_label() {
    let mut t = tab(3, "Editor");
    t.off_workspace = true;
    t.icons = vec!["Ghostty".into(), "Chrome".into()];
    t.badges = vec![false, true];
    let l = lay(&[t], 0, true, &Mark::Logo);
    let runs = &l.tabs[0].content;

    assert!(matches!(&runs[0], Run::Text { color: Role::Accent, .. }), "the workspace marker leads");
    assert!(matches!(&runs[1], Run::Text { text, color: Role::Dim, .. } if text == "3 "));
    assert!(matches!(&runs[2], Run::Icon { app, badge: false, .. } if app == "Ghostty"));
    assert!(matches!(&runs[3], Run::Icon { app, badge: true, .. } if app == "Chrome"));
    assert!(matches!(&runs[4], Run::Text { text, .. } if text == "Editor"));

    let xs: Vec<f64> = runs
        .iter()
        .map(|r| match r {
            Run::Text { x, .. } | Run::Icon { x, .. } => *x,
        })
        .collect();
    assert!(xs.windows(2).all(|w| w[0] < w[1]), "runs must not overlap or backtrack");
    assert!(xs[0] >= l.tabs[0].x0 && *xs.last().unwrap() < l.tabs[0].x1);
}

#[test]
fn a_zoomed_tab_is_marked_and_an_iconless_label_still_lays_out() {
    let mut t = tab(1, "Editor");
    t.zoomed = true;
    let l = lay(&[t], 0, true, &Mark::Logo);
    // The mark leads in the accent color, like the off-workspace one; the label keeps its own text.
    let Some(Run::Text { text, color, .. }) = l.tabs[0].content.first() else { panic!("a marker leads the tab") };
    assert!(text.contains(crate::chrome::ZOOM_MARK));
    assert_eq!(*color, Role::Accent);
    let Some(Run::Text { text, .. }) = l.tabs[0].content.last() else { panic!("the label is the last run") };
    assert_eq!(text, "Editor");

    // Both states at once share one leading run: on another workspace, and zoomed.
    let mut both = tab(2, "Editor");
    both.zoomed = true;
    both.off_workspace = true;
    let l2 = lay(&[both], 0, true, &Mark::Logo);
    let Some(Run::Text { text, .. }) = l2.tabs[0].content.first() else { panic!("a marker leads the tab") };
    assert_eq!(*text, format!("{}{} ", crate::chrome::WORKSPACE_MARK, crate::chrome::ZOOM_MARK));

    // A whitespace-only custom name renders as just the icon: no label run, and a narrower tab.
    let mut bare = tab(1, "");
    bare.icons = vec!["Ghostty".into()];
    let narrow = lay(&[bare], 0, true, &Mark::Logo);
    assert!(narrow.tabs[0].content.iter().all(|r| !matches!(r, Run::Text { text, .. } if text == "Editor")));
    assert!(narrow.tabs[0].x1 - narrow.tabs[0].x0 < l.tabs[0].x1 - l.tabs[0].x0);
}

#[test]
fn a_user_glyph_sizes_its_own_slot_and_the_logo_uses_a_fixed_one() {
    let tabs = [tab(1, "one")];
    let logo = lay(&tabs, 0, true, &Mark::Logo).lead.unwrap();
    let narrow = lay(&tabs, 0, true, &Mark::Glyph("*".into())).lead.unwrap();
    let wide = lay(&tabs, 0, true, &Mark::Glyph("wide-glyph".into())).lead.unwrap();

    assert!(matches!(logo.glyph, LeadGlyph::Logo(_)));
    // A narrow glyph floors at the minimum slot, under the logo's fixed one; a wide one grows past both.
    assert!(narrow.width < logo.width);
    assert!(wide.width > logo.width);
    let LeadGlyph::Glyph { x, .. } = wide.glyph else { panic!("a glyph mark draws as text") };
    assert!(x > 0.0 && x < wide.width);
}

#[test]
fn trailing_app_icons_take_their_room_from_the_tabs() {
    let tabs = [tab(1, "one")];
    let apps: Vec<String> = vec!["Notes".into(), "Music".into()];
    let bare = lay(&tabs, 0, true, &Mark::Logo);
    let trailed = lay_with(&tabs, 0, true, &Mark::Logo, &apps);

    // Two icons, left to right, ending before the prefix dot, each one clickable.
    assert_eq!(trailed.apps.len(), 2);
    assert!(trailed.apps[0] < trailed.apps[1]);
    let (_, last) = *trailed.app_hits().last().unwrap();
    assert!(last < trailed.dot.unwrap().0, "the icons stop short of the dot");
    // The tabs' clip shrinks to make room, and the tabs themselves are laid out the same.
    assert!(trailed.content_w < bare.content_w);
    assert_eq!(trailed.tabs, bare.tabs);
    // A stack bar carries none.
    assert!(lay_with(&tabs, 0, false, &Mark::Logo, &apps).apps.is_empty());
}
