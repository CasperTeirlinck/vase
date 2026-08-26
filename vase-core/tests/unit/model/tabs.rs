use super::*;

#[test]
fn next_tab_and_prev_tab_wrap() {
    let (m, effects) = apply(three(), Command::NextTab);
    assert_eq!(m.screens[0].current, 1);
    assert!(matches!(effects.first(), Some(Effect::Render(_))));
    assert!(effects.contains(&Effect::FocusWindow(win(2))));
    let (m, _) = apply(m, Command::PrevTab);
    assert_eq!(m.screens[0].current, 0);
    let (m, _) = apply(m, Command::PrevTab);
    assert_eq!(m.screens[0].current, 2);
    assert_eq!(m.focused_window(), Some(win(3)));
}

#[test]
fn select_tab_sets_current_and_out_of_range_is_a_noop() {
    let (m, effects) = apply(three(), Command::SelectTab(2));
    assert_eq!(m.screens[0].current, 2);
    assert!(matches!(effects.first(), Some(Effect::Render(_))));
    assert!(effects.contains(&Effect::FocusWindow(win(3))));
    let (m2, effects) = apply(m.clone(), Command::SelectTab(9));
    assert_eq!(m2.screens[0].current, m.screens[0].current);
    assert_eq!(effects, vec![]);
}

#[test]
fn move_tab_reorders_and_clamps_at_the_boundary() {
    let (m, effects) = apply(three(), Command::MoveTab(1));
    assert_eq!(m.screens[0].current, 1);
    let reps: Vec<_> = m.bar_tabs().0.iter().map(|(_, rep, _, _)| *rep).collect();
    assert_eq!(reps, vec![Some(win(2)), Some(win(1)), Some(win(3))]);
    assert_eq!(effects, vec![]);
    // Already at the front: moving left is a no-op.
    let (m2, effects) = apply(three(), Command::MoveTab(-1));
    assert_eq!(m2.screens[0].tabs, three().screens[0].tabs);
    assert_eq!(effects, vec![]);
}

#[test]
fn bar_tabs_reports_a_representative_window_per_tab() {
    let (m, _) = apply(three(), Command::Split(Dir::Horizontal));
    // Tab 0 now has an empty focused pane; its representative is window 1.
    let (tabs, current) = m.bar_tabs();
    let reps: Vec<_> = tabs.iter().map(|(_, rep, _, _)| *rep).collect();
    assert_eq!(reps, vec![Some(win(1)), Some(win(2)), Some(win(3))]);
    // Tab 0's icons list still holds its window (win1).
    assert_eq!(tabs[0].0, vec![win(1)]);
    assert_eq!(current, 0);
}

#[test]
fn set_tab_name_sets_and_clears_the_current_tab_name() {
    let (m, _) = apply(three(), Command::SetTabName(Some("build".into())));
    assert_eq!(m.bar_tabs().0[0].2.as_deref(), Some("build"));
    // A whitespace-only name is kept (the bar renders it as icon-only).
    let (m, _) = apply(m, Command::SetTabName(Some(" ".into())));
    assert_eq!(m.bar_tabs().0[0].2.as_deref(), Some(" "));
    // An empty name / None clears the override (title label returns).
    let (m, _) = apply(m, Command::SetTabName(None));
    assert_eq!(m.bar_tabs().0[0].2, None);
}

#[test]
fn a_named_window_keeps_its_name_across_a_move_into_a_split_and_back() {
    // Name win 2's tab, then move win 2 into another tab's split and break it back out.
    let (m, _) = apply(three(), Command::SelectTab(1));
    let (m, _) = apply(m, Command::SetTabName(Some("bar".into())));
    assert_eq!(m.names.get(&win(2)), Some(&"bar".to_string()));
    let (m, _) = apply(m, Command::SelectTab(0));
    let (m, _) = apply(m, Command::Split(Dir::Horizontal));
    let (m, _) = apply(m, Command::FillPane(win(2))); // win 2's own tab is destroyed here
                                                      // The name survives the move: it is keyed by the window, not the destroyed tab.
    assert_eq!(m.names.get(&win(2)), Some(&"bar".to_string()));
    let (m, _) = apply(m, Command::BreakPane);
    let (tabs, _) = m.bar_tabs();
    let win2_tab = tabs.iter().find(|(ws, _, _, _)| ws == &vec![win(2)]).expect("win 2 has its own tab again");
    assert_eq!(win2_tab.2.as_deref(), Some("bar"));
}

#[test]
fn a_zoom_stays_with_the_tab_it_was_made_in() {
    // Two tabs, each a split. Zoom the first one's focused pane.
    let mut m = one(&[win(1)]);
    m.screens[0].tabs[0].root = Node::Split {
        dir: Dir::Horizontal,
        ratios: vec![0.5, 0.5],
        children: vec![Node::Leaf { id: PaneId(0), pane: Pane::Window(win(1)) }, Node::Leaf { id: PaneId(1), pane: Pane::Window(win(2)) }],
    };
    m.screens[0].tabs.push(Tab::single(PaneId(2), Pane::Window(win(3))));
    let (m, _) = apply(m, Command::ToggleZoom);
    assert_eq!(m.placements(), vec![(win(1), SCREEN)], "the zoomed pane owns the screen");

    // Move to the other tab: it lays out normally, and only the tab left behind is marked.
    let (m, _) = apply(m, Command::SelectTab(1));
    assert_eq!(m.placements(), vec![(win(3), SCREEN)]);
    let marks: Vec<bool> = m.bar_tabs().0.iter().map(|(_, _, _, zoomed)| *zoomed).collect();
    assert_eq!(marks, vec![true, false], "the mark stays on the tab that is zoomed");

    // Back again, and the zoom is where it was left.
    let (m, _) = apply(m, Command::SelectTab(0));
    assert_eq!(m.placements(), vec![(win(1), SCREEN)]);
}
