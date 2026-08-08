use super::*;

#[test]
fn next_tab_cycles_across_all_monitors() {
    let (m, _) = apply(two_screens(), Command::NextTab);
    assert_eq!(m.focused_screen, 1);
    assert_eq!(m.focused_window(), Some(win(2)));
    // Wraps back to the first monitor's tab.
    let (m, _) = apply(m, Command::NextTab);
    assert_eq!(m.focused_screen, 0);
    assert_eq!(m.focused_window(), Some(win(1)));
    // PrevTab from the first tab wraps to the last monitor's tab.
    let (m, _) = apply(m, Command::PrevTab);
    assert_eq!(m.focused_screen, 1);
    assert_eq!(m.focused_window(), Some(win(2)));
}

#[test]
fn move_tab_carries_the_tab_to_the_next_monitor() {
    let (m, effects) = apply(two_screens(), Command::MoveTab(1));
    assert!(m.screens[0].tabs.is_empty());
    assert_eq!(windows(&m.screens[1].tabs[0].root), vec![win(1)]);
    assert_eq!(windows(&m.screens[1].tabs[1].root), vec![win(2)]);
    assert_eq!(m.focused_screen, 1);
    assert_eq!(m.focused_window(), Some(win(1)));
    // Physically relocates a window → renders.
    assert!(effects.iter().any(|e| matches!(e, Effect::Render(_))));
}

#[test]
fn move_tab_to_screen_sends_the_tab_across_monitors_and_wraps() {
    // win(1) is screen 0's only tab, focused.
    let (m, _) = apply(two_screens(), Command::MoveTabToScreen(1));
    assert_eq!(m.screens[0].tabs.len(), 0);
    assert_eq!(m.focused_screen, 1);
    assert_eq!(m.screens[1].current, 0); // inserted at the near (front) edge
    assert_eq!(m.focused_window(), Some(win(1)));
    // From the right screen, moving right again wraps back to screen 0.
    let (m, _) = apply(m, Command::MoveTabToScreen(1));
    assert_eq!(m.focused_screen, 0);
    assert_eq!(m.focused_window(), Some(win(1)));
}

#[test]
fn move_tab_to_screen_is_a_noop_on_a_single_monitor() {
    let m = three();
    let (m2, effects) = apply(m.clone(), Command::MoveTabToScreen(1));
    assert_eq!(m2, m);
    assert_eq!(effects, vec![]);
}

#[test]
fn focus_crosses_to_the_monitor_on_the_right() {
    let (m, _) = apply(two_screens(), Command::Focus(Direction::Right));
    assert_eq!(m.focused_screen, 1);
    assert_eq!(m.focused_window(), Some(win(2)));
    let (m, _) = apply(m, Command::Focus(Direction::Left));
    assert_eq!(m.focused_screen, 0);
    assert_eq!(m.focused_window(), Some(win(1)));
}

#[test]
fn move_window_across_monitors_swaps_positions() {
    let (m, _) = apply(two_screens(), Command::MoveWindow(Direction::Right));
    assert_eq!(m.focused_screen, 1);
    assert_eq!(m.focused_window(), Some(win(1)));
    let s1 = &m.screens[1];
    let s0 = &m.screens[0];
    assert_eq!(windows(&s1.tabs[s1.current].root), vec![win(1)]);
    assert_eq!(windows(&s0.tabs[s0.current].root), vec![win(2)]);
}

#[test]
fn placements_span_all_screens() {
    let p = two_screens().placements();
    assert!(p.contains(&(win(1), Rect::new(0.0, 0.0, 100.0, 100.0))));
    assert!(p.contains(&(win(2), Rect::new(100.0, 0.0, 100.0, 100.0))));
}

#[test]
fn zoom_only_affects_focused_screen() {
    // Screen 0 holds a win1 | win3 split (focused on win1); screen 1 holds win2.
    let mut m = two_screens();
    m.screens[0].tabs[0].root = Node::Split {
        dir: Dir::Horizontal,
        ratios: vec![0.5, 0.5],
        children: vec![Node::Leaf { id: PaneId(0), pane: Pane::Window(win(1)) }, Node::Leaf { id: PaneId(2), pane: Pane::Window(win(3)) }],
    };
    m.screens[0].tabs[0].focused = PaneId(0);
    m.next_pane_id = 3;
    // Before zoom, win1 only tiles into its half of screen 0.
    assert!(m.placements().contains(&(win(1), Rect::new(0.0, 0.0, 50.0, 100.0))));

    let (m, _) = apply(m, Command::ToggleZoom);
    let p = m.placements();
    // Zoom fills the focused window over screen 0 and hides its split sibling win3...
    assert!(p.contains(&(win(1), Rect::new(0.0, 0.0, 100.0, 100.0))));
    assert!(!p.iter().any(|(w, _)| *w == win(3)));
    // ...while screen 1 stays tiled normally.
    assert!(p.contains(&(win(2), Rect::new(100.0, 0.0, 100.0, 100.0))));
}
