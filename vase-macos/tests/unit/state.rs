use crate::state::*;
use vase_core::geometry::Rect;
use vase_core::model::Model;
use vase_core::tree::{windows, WindowId};

fn rect() -> Rect {
    Rect::new(0.0, 0.0, 1000.0, 800.0)
}

fn live(id: u64, app: &str, title: &str) -> LiveWindow {
    LiveWindow { id: WindowId(id), app: app.into(), title: title.into(), screen: 0 }
}

fn ident(id: u64, app: &str, title: &str) -> WindowIdentity {
    (WindowId(id), app.into(), title.into())
}

/// Every window in the model, in tab order.
fn tabbed(m: &Model) -> Vec<Vec<WindowId>> {
    m.screens.iter().flat_map(|s| s.tabs.iter()).map(|t| windows(&t.root)).collect()
}

/// A two-tab saved layout: Ghostty then Chrome, the second tab renamed.
fn saved() -> (Model, Vec<WindowIdentity>) {
    let mut m = Model::adopt(&[rect()], &[(WindowId(1), 0), (WindowId(2), 0)]);
    m.screens[0].tabs[1].name = Some("web".into());
    let ids = vec![ident(1, "Ghostty", "~/src"), ident(2, "Google Chrome", "Inbox")];
    (m, ids)
}

#[test]
fn no_saved_layout_gives_one_tab_per_live_window() {
    let m = restore(None, &[live(7, "Ghostty", "~"), live(8, "Safari", "News")], &[rect()]);
    assert_eq!(tabbed(&m), vec![vec![WindowId(7)], vec![WindowId(8)]]);
}

#[test]
fn same_session_ids_are_kept_and_tab_order_survives() {
    let m = restore(Some(saved()), &[live(2, "Google Chrome", "Inbox"), live(1, "Ghostty", "~/src")], &[rect()]);
    assert_eq!(tabbed(&m), vec![vec![WindowId(1)], vec![WindowId(2)]], "saved order wins over live order");
    assert_eq!(m.screens[0].tabs[1].name.as_deref(), Some("web"), "the custom name survives");
}

#[test]
fn after_a_reboot_windows_are_rematched_by_app_and_title() {
    // Every id was reassigned; titles are unchanged.
    let m = restore(Some(saved()), &[live(91, "Google Chrome", "Inbox"), live(90, "Ghostty", "~/src")], &[rect()]);
    assert_eq!(tabbed(&m), vec![vec![WindowId(90)], vec![WindowId(91)]]);
    assert_eq!(m.screens[0].tabs[1].name.as_deref(), Some("web"));
}

#[test]
fn a_changed_title_still_matches_on_the_app_alone() {
    let m = restore(Some(saved()), &[live(90, "Ghostty", "~/other"), live(91, "Google Chrome", "Gmail")], &[rect()]);
    assert_eq!(tabbed(&m), vec![vec![WindowId(90)], vec![WindowId(91)]]);
}

#[test]
fn an_exact_title_match_beats_a_bare_app_match() {
    let mut m = Model::adopt(&[rect()], &[(WindowId(1), 0)]);
    m.screens[0].tabs[0].name = Some("inbox tab".into());
    let ids = vec![ident(1, "Google Chrome", "Inbox")];
    // Two live Chrome windows; the one whose title matches must win.
    let m = restore(Some((m, ids)), &[live(90, "Google Chrome", "Gmail"), live(91, "Google Chrome", "Inbox")], &[rect()]);
    assert_eq!(tabbed(&m)[0], vec![WindowId(91)]);
}

#[test]
fn each_live_window_is_claimed_by_at_most_one_saved_tab() {
    let mut m = Model::adopt(&[rect()], &[(WindowId(1), 0), (WindowId(2), 0)]);
    m.screens[0].tabs[0].name = Some("a".into());
    let ids = vec![ident(1, "Ghostty", "one"), ident(2, "Ghostty", "two")];
    // Only one Ghostty is left: the first saved tab takes it, the second is pruned.
    let m = restore(Some((m, ids)), &[live(90, "Ghostty", "one")], &[rect()]);
    assert_eq!(tabbed(&m), vec![vec![WindowId(90)]]);
}

#[test]
fn a_saved_window_with_no_live_match_is_pruned() {
    let m = restore(Some(saved()), &[live(1, "Ghostty", "~/src")], &[rect()]);
    assert_eq!(tabbed(&m), vec![vec![WindowId(1)]], "the Chrome tab is gone");
}

#[test]
fn a_live_window_no_saved_tab_claimed_becomes_a_new_tab() {
    let m = restore(Some(saved()), &[live(1, "Ghostty", "~/src"), live(2, "Google Chrome", "Inbox"), live(3, "Obsidian", "notes")], &[rect()]);
    assert_eq!(tabbed(&m), vec![vec![WindowId(1)], vec![WindowId(2)], vec![WindowId(3)]]);
}

#[test]
fn a_saved_layout_adapts_to_a_different_screen_count() {
    let two = [rect(), Rect::new(1000.0, 0.0, 1000.0, 800.0)];
    let mut m = Model::adopt(&two, &[(WindowId(1), 0), (WindowId(2), 1)]);
    m.screens[1].tabs[0].name = Some("second screen".into());
    let ids = vec![ident(1, "Ghostty", "~"), ident(2, "Safari", "News")];
    // Down to one screen: the second screen's tabs migrate rather than vanish.
    let m = restore(Some((m, ids)), &[live(1, "Ghostty", "~"), live(2, "Safari", "News")], &[rect()]);
    assert_eq!(m.screens.len(), 1);
    assert_eq!(tabbed(&m), vec![vec![WindowId(1)], vec![WindowId(2)]]);
}

#[test]
fn names_follow_their_window_through_a_rematch() {
    let mut m = Model::adopt(&[rect()], &[(WindowId(1), 0)]);
    m.names.insert(WindowId(1), "editor".into());
    let ids = vec![ident(1, "Ghostty", "~")];
    let m = restore(Some((m, ids)), &[live(90, "Ghostty", "~")], &[rect()]);
    assert_eq!(m.names.get(&WindowId(90)).map(String::as_str), Some("editor"));
}
