use crate::daemon::{app_matches, clean_title, screen_of};
use vase_core::geometry::Rect;

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
fn screen_of_places_a_frame_by_its_center() {
    let screens = [Rect::new(0.0, 0.0, 1000.0, 800.0), Rect::new(1000.0, 0.0, 1000.0, 800.0)];
    assert_eq!(screen_of(Rect::new(10.0, 10.0, 100.0, 100.0), &screens), 0);
    assert_eq!(screen_of(Rect::new(1400.0, 10.0, 100.0, 100.0), &screens), 1);
    // A window straddling the seam belongs to whichever side holds its center.
    assert_eq!(screen_of(Rect::new(900.0, 10.0, 400.0, 100.0), &screens), 1);
    // Off every display falls back to the first.
    assert_eq!(screen_of(Rect::new(-9000.0, 0.0, 10.0, 10.0), &screens), 0);
}

#[test]
fn app_matches_either_direction_and_ignores_case() {
    assert!(app_matches("Google Chrome", "google chrome"));
    assert!(app_matches("Brave Browser", "Brave"), "configured short name matches the full one");
    assert!(app_matches("Code", "Visual Studio Code"), "and the other way round");
    assert!(!app_matches("Ghostty", "Obsidian"));
}
