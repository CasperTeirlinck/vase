use crate::backend::WindowInfo;
use crate::geometry::Rect;
use crate::registry::*;
use crate::tree::WindowId;

fn info(id: u64, app: &str, title: &str) -> WindowInfo {
    WindowInfo { id: WindowId(id), pid: 1, app: app.into(), title: title.into(), frame: Rect::new(10.0, 20.0, 300.0, 400.0), layer: 0 }
}

#[test]
fn adopting_records_every_field_at_once() {
    let mut r = Registry::default();
    r.adopt(&info(1, "Ghostty", "~/src"), false);
    let w = r.get(WindowId(1)).unwrap();
    assert_eq!(w.app, "Ghostty");
    assert_eq!(w.title, "~/src");
    assert_eq!(w.original, Rect::new(10.0, 20.0, 300.0, 400.0));
    assert!(!w.minimized);
    assert_eq!(w.placed, None);
}

#[test]
fn forgetting_drops_everything_keyed_to_the_window() {
    let mut r = Registry::default();
    r.adopt(&info(1, "Ghostty", "~"), true);
    r.get_mut(WindowId(1)).unwrap().placed = Some(Rect::new(0.0, 0.0, 1.0, 1.0));
    r.forget(WindowId(1));
    assert!(!r.contains(WindowId(1)));
    assert_eq!(r.app(WindowId(1)), "");
    assert_eq!(r.title(WindowId(1)), "");
    assert!(!r.is_minimized(WindowId(1)));
    assert!(r.is_empty());
}

#[test]
fn re_adopting_refreshes_the_title_but_keeps_the_original_frame() {
    let mut r = Registry::default();
    r.adopt(&info(1, "Ghostty", "~/src"), false);
    let mut moved = info(1, "Ghostty", "~/other");
    moved.frame = Rect::new(999.0, 999.0, 100.0, 100.0);
    r.adopt(&moved, false);
    let w = r.get(WindowId(1)).unwrap();
    assert_eq!(w.title, "~/other");
    assert_eq!(w.original, Rect::new(10.0, 20.0, 300.0, 400.0), "exit must still restore the first frame");
}

#[test]
fn re_adopting_does_not_resurrect_a_stale_placement() {
    let mut r = Registry::default();
    r.adopt(&info(1, "Ghostty", "~"), false);
    r.get_mut(WindowId(1)).unwrap().placed = Some(Rect::new(5.0, 5.0, 5.0, 5.0));
    r.adopt(&info(1, "Ghostty", "~"), false);
    assert_eq!(r.get(WindowId(1)).unwrap().placed, Some(Rect::new(5.0, 5.0, 5.0, 5.0)), "placement survives");
}

#[test]
fn set_title_reports_only_real_changes() {
    let mut r = Registry::default();
    r.adopt(&info(1, "Ghostty", "~"), false);
    assert!(r.set_title(WindowId(1), "~/src".into()));
    assert!(!r.set_title(WindowId(1), "~/src".into()), "same title is not a change");
    assert!(!r.set_title(WindowId(9), "anything".into()), "unknown window is not a change");
}

#[test]
fn minimized_state_survives_a_round_trip() {
    let mut r = Registry::default();
    r.adopt(&info(1, "Ghostty", "~"), true);
    assert!(r.is_minimized(WindowId(1)));
    r.set_minimized(WindowId(1), false);
    assert!(!r.is_minimized(WindowId(1)));
}

#[test]
fn clean_title_strips_the_redundant_app_name() {
    // App-name prefix (Activity Monitor) → keep the meaningful remainder.
    assert_eq!(clean_title("Activity Monitor – All Processes", "Activity Monitor"), "All Processes");
    // App name mid/suffix (Chrome/Brave) → keep the part before it.
    assert_eq!(clean_title("Incidents - PagerDuty - Google Chrome – Person 1", "Google Chrome"), "Incidents - PagerDuty");
    // App's first word matches when the full name doesn't (Brave Browser → Brave).
    assert_eq!(clean_title("Workflow runs · checkup - Brave", "Brave Browser"), "Workflow runs · checkup");
    // Title that is only the app name → nothing left.
    assert_eq!(clean_title("Ghostty", "Ghostty"), "");
    // Unrelated title is left as-is.
    assert_eq!(clean_title("notes.md — draft", "Obsidian"), "notes.md — draft");
}

#[test]
fn app_matches_either_direction_and_ignores_case() {
    assert!(app_matches("Google Chrome", "google chrome"));
    assert!(app_matches("Brave Browser", "Brave"), "configured short name matches the full one");
    assert!(app_matches("Code", "Visual Studio Code"), "and the other way round");
    assert!(!app_matches("Ghostty", "Obsidian"));
}
