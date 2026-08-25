use super::*;

#[test]
fn adopt_makes_one_single_pane_tab_per_window() {
    let m = three();
    assert_eq!(m.screens[0].tabs.len(), 3);
    assert_eq!(m.screens[0].current, 0);
    assert_eq!(m.next_pane_id, 3);
    assert_eq!(m.screens[0].tabs[0].root, Node::Leaf { id: PaneId(0), pane: Pane::Window(win(1)) });
    assert_eq!(m.screens[0].tabs[0].focused, PaneId(0));
    assert_eq!(m.placements(), vec![(win(1), SCREEN)]);
}

#[test]
fn adopt_empty_yields_no_tabs() {
    let m = one(&[]);
    assert!(m.screens[0].tabs.is_empty());
    assert_eq!(m.screens[0].current, 0);
    assert_eq!(m.placements(), vec![]);
}

#[test]
fn add_window_opens_a_new_current_tab() {
    let (m, effects) = apply(three(), Command::AddWindow(win(4), 0));
    assert_eq!(m.screens[0].tabs.len(), 4);
    assert_eq!(m.screens[0].current, 3);
    assert_eq!(m.focused_window(), Some(win(4)));
    assert_eq!(effects, vec![Effect::Render(vec![(win(4), SCREEN)]), Effect::FocusWindow(win(4))]);
}

#[test]
fn new_tab_appends_an_empty_focused_tab() {
    let (m, effects) = apply(three(), Command::NewTab);
    assert_eq!(m.screens[0].tabs.len(), 4);
    assert_eq!(m.screens[0].current, 3); // the new tab is current
    assert!(m.focused_pane_is_empty()); // its pane is empty → picker opens
                                        // Empty pane means no window to focus, just a re-tile.
    assert!(matches!(effects.first(), Some(Effect::Render(_))));
    assert!(!effects.iter().any(|e| matches!(e, Effect::FocusWindow(_))));
}

#[test]
fn all_placements_covers_the_tabs_no_one_is_looking_at() {
    // Three single-window tabs on one screen: only the current one is on screen.
    let m = three();
    assert_eq!(m.placements(), vec![(win(1), SCREEN)], "only the current tab is rendered");

    // Every tab lays its own window out on the same screen rect, current or not.
    let all = m.all_placements();
    assert_eq!(all.len(), 3);
    assert!(all.iter().all(|(_, rect)| *rect == SCREEN));
    for w in [win(1), win(2), win(3)] {
        assert!(all.iter().any(|(id, _)| *id == w), "{w:?} is in the model, so resync has a rect for it");
    }

    // A zoom is about what is on screen, so it does not reach the tabs behind it.
    let mut zoomed = h_split(false);
    zoomed.zoomed = true;
    assert_eq!(zoomed.placements().len(), 1, "the zoomed pane owns the screen");
    assert_eq!(zoomed.all_placements().len(), 2, "both panes still have a layout rect");
}
