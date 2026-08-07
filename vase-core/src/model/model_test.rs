use super::*;

const SCREEN: Rect = Rect { x: 0.0, y: 0.0, w: 100.0, h: 100.0 };

fn win(id: u64) -> WindowId {
    WindowId(id)
}

/// Single-screen model: every window on screen 0.
fn one(ws: &[WindowId]) -> Model {
    Model::adopt(&[SCREEN], &ws.iter().map(|w| (*w, 0)).collect::<Vec<_>>())
}

fn three() -> Model {
    one(&[win(1), win(2), win(3)])
}

/// Two screens side by side in global coords: screen 0 at x[0,100], screen 1
/// at x[100,200], same height — so `neighbor` can cross between them.
fn two_screens() -> Model {
    Model::adopt(
        &[Rect { x: 0.0, y: 0.0, w: 100.0, h: 100.0 }, Rect { x: 100.0, y: 0.0, w: 100.0, h: 100.0 }],
        &[(win(1), 0), (win(2), 1)],
    )
}

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
    assert_eq!(
        effects,
        vec![Effect::Render(vec![(win(4), SCREEN)]), Effect::FocusWindow(win(4))]
    );
}

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
fn split_makes_an_empty_focused_pane_and_reflows_the_window() {
    let (m, effects) = apply(three(), Command::Split(Dir::Horizontal));
    // The new empty pane is focused (a fresh PaneId), no window under focus.
    assert_eq!(m.focused_window(), None);
    assert_eq!(
        m.screens[0].tabs[0].root,
        Node::Split {
            dir: Dir::Horizontal,
            ratios: vec![0.5, 0.5],
            children: vec![
                Node::Leaf { id: PaneId(0), pane: Pane::Window(win(1)) },
                Node::Leaf { id: PaneId(3), pane: Pane::Empty },
            ],
        }
    );
    assert_eq!(m.screens[0].tabs[0].focused, PaneId(3));
    // Only the window reflows into its half; NO FocusWindow (empty focus).
    assert_eq!(effects, vec![Effect::Render(vec![(win(1), Rect::new(0.0, 0.0, 50.0, 100.0))])]);
    assert_eq!(m.empty_panes(), vec![(Rect::new(50.0, 0.0, 50.0, 100.0), true)]);
}

#[test]
fn focus_moves_onto_an_empty_pane_without_focuswindow() {
    let (m, _) = apply(three(), Command::Split(Dir::Horizontal));
    // Focus is on the empty pane; move Left back onto the window pane.
    let (m, effects) = apply(m, Command::Focus(Direction::Left));
    assert_eq!(m.focused_window(), Some(win(1)));
    assert_eq!(effects, vec![Effect::FocusWindow(win(1))]);
    // Move Right onto the empty pane: focus changes, no FocusWindow.
    let (m, effects) = apply(m, Command::Focus(Direction::Right));
    assert_eq!(m.focused_window(), None);
    assert_eq!(effects, vec![]);
}

#[test]
fn focus_at_the_edge_is_a_noop() {
    let m = three();
    let (m2, effects) = apply(m.clone(), Command::Focus(Direction::Left));
    assert_eq!(m2, m);
    assert_eq!(effects, vec![]);
}

#[test]
fn move_window_swaps_panes_and_focus_follows_the_window() {
    // Split window 1, then move it Right into the empty pane's slot.
    let (m, _) = apply(one(&[win(1)]), Command::Split(Dir::Horizontal));
    let (m, _) = apply(m, Command::Focus(Direction::Left)); // focus the window pane
    let (m, effects) = apply(m, Command::MoveWindow(Direction::Right));
    // Window 1 now lives in the right pane (PaneId 1), focus followed it.
    assert_eq!(m.screens[0].tabs[0].focused, PaneId(1));
    assert_eq!(m.focused_window(), Some(win(1)));
    assert_eq!(
        m.screens[0].tabs[0].root,
        Node::Split {
            dir: Dir::Horizontal,
            ratios: vec![0.5, 0.5],
            children: vec![
                Node::Leaf { id: PaneId(0), pane: Pane::Empty },
                Node::Leaf { id: PaneId(1), pane: Pane::Window(win(1)) },
            ],
        }
    );
    assert!(effects.contains(&Effect::Render(vec![(win(1), Rect::new(50.0, 0.0, 50.0, 100.0))])));
    assert!(effects.contains(&Effect::FocusWindow(win(1))));
}

#[test]
fn move_window_at_the_edge_is_a_noop() {
    let (m, _) = apply(one(&[win(1)]), Command::Split(Dir::Horizontal));
    let (m, _) = apply(m, Command::Focus(Direction::Left));
    let (m2, effects) = apply(m.clone(), Command::MoveWindow(Direction::Left));
    assert_eq!(m2.screens[0].tabs[0].root, m.screens[0].tabs[0].root);
    assert_eq!(effects, vec![]);
}

#[test]
fn move_tab_reorders_and_clamps_at_the_boundary() {
    let (m, effects) = apply(three(), Command::MoveTab(1));
    assert_eq!(m.screens[0].current, 1);
    let reps: Vec<_> = m.bar_tabs().0.iter().map(|(_, r, _)| *r).collect();
    assert_eq!(reps, vec![Some(win(2)), Some(win(1)), Some(win(3))]);
    assert_eq!(effects, vec![]);
    // Already at the front: moving left is a no-op.
    let (m2, effects) = apply(three(), Command::MoveTab(-1));
    assert_eq!(m2.screens[0].tabs, three().screens[0].tabs);
    assert_eq!(effects, vec![]);
}

#[test]
fn next_tab_cycles_across_all_monitors() {
    // Two monitors, one tab each; cycling spans both in bar order.
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
    // Moving right past screen 0's only tab relocates it onto screen 1.
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
fn retain_windows_prunes_dead_windows_and_empty_tabs() {
    let mut m = two_screens(); // win1 on screen 0, win2 on screen 1
    m.retain_windows(&HashSet::from([win(1)]));
    assert_eq!(windows(&m.screens[0].tabs[0].root), vec![win(1)]);
    assert!(m.screens[1].tabs.is_empty()); // win2's tab dropped
}

#[test]
fn retain_windows_collapses_a_split_around_a_dead_window() {
    let mut m = h_split(false); // one tab, split win1 | win2 on screen 0
    m.retain_windows(&HashSet::from([win(2)]));
    // The split collapses to just win2.
    assert_eq!(m.screens[0].tabs.len(), 1);
    assert_eq!(windows(&m.screens[0].tabs[0].root), vec![win(2)]);
}

#[test]
fn reconfigure_shrinks_and_migrates_tabs_to_the_last_screen() {
    let m = &mut two_screens();
    let one_rect = [Rect { x: 0.0, y: 0.0, w: 100.0, h: 100.0 }];
    m.reconfigure(&one_rect);
    assert_eq!(m.screens.len(), 1);
    // Both windows now live on the single remaining screen.
    assert_eq!(windows(&m.screens[0].tabs[0].root), vec![win(1)]);
    assert_eq!(windows(&m.screens[0].tabs[1].root), vec![win(2)]);
}

#[test]
fn remap_windows_renames_survivors_and_drops_unmapped() {
    let mut m = two_screens(); // win1 on screen 0, win2 on screen 1
    // win1 -> win10 (reboot reassigned its id); win2 has no live match.
    let map = HashMap::from([(win(1), win(10))]);
    m.remap_windows(&map);
    assert_eq!(windows(&m.screens[0].tabs[0].root), vec![win(10)]);
    assert!(m.screens[1].tabs.is_empty());
}

#[test]
fn reconfigure_grows_with_empty_screens_and_updates_rects() {
    let m = &mut one(&[win(1)]);
    let rects = [
        Rect { x: 0.0, y: 0.0, w: 100.0, h: 100.0 },
        Rect { x: 100.0, y: 0.0, w: 200.0, h: 200.0 },
    ];
    m.reconfigure(&rects);
    assert_eq!(m.screens.len(), 2);
    assert!(m.screens[1].tabs.is_empty());
    assert_eq!(m.screens[1].rect, rects[1]);
}

fn h_split(focus_right: bool) -> Model {
    // A single tab holding two side-by-side windows.
    let mut m = one(&[win(1)]);
    m.screens[0].tabs[0].root = Node::Split {
        dir: Dir::Horizontal,
        ratios: vec![0.5, 0.5],
        children: vec![
            Node::Leaf { id: PaneId(0), pane: Pane::Window(win(1)) },
            Node::Leaf { id: PaneId(1), pane: Pane::Window(win(2)) },
        ],
    };
    m.screens[0].tabs[0].focused = if focus_right { PaneId(1) } else { PaneId(0) };
    m.next_pane_id = 2;
    m
}

fn split_ratios(node: &Node) -> Vec<f64> {
    match node {
        Node::Split { ratios, .. } => ratios.clone(),
        _ => panic!("not a split"),
    }
}

#[test]
fn resize_right_grows_the_focused_left_pane() {
    let (m, effects) = apply(h_split(false), Command::Resize(Direction::Right));
    let r = split_ratios(&m.screens[0].tabs[0].root);
    assert!((r[0] - 0.55).abs() < 1e-9 && (r[1] - 0.45).abs() < 1e-9);
    assert!(matches!(effects[0], Effect::Render(_)));
    let (m, _) = apply(m, Command::Resize(Direction::Left));
    let r = split_ratios(&m.screens[0].tabs[0].root);
    assert!((r[0] - 0.5).abs() < 1e-9 && (r[1] - 0.5).abs() < 1e-9);
}

#[test]
fn resize_right_from_the_right_pane_shrinks_it() {
    // Same divider motion whichever pane is focused.
    let (m, _) = apply(h_split(true), Command::Resize(Direction::Right));
    let r = split_ratios(&m.screens[0].tabs[0].root);
    assert!((r[0] - 0.55).abs() < 1e-9 && (r[1] - 0.45).abs() < 1e-9);
}

#[test]
fn resize_clamps_at_min_ratio() {
    let mut m = h_split(false);
    for _ in 0..20 {
        let (nm, _) = apply(m, Command::Resize(Direction::Right));
        m = nm;
    }
    let r = split_ratios(&m.screens[0].tabs[0].root);
    assert!(r[1] >= MIN_RATIO - 1e-9);
    assert!(r[0] <= 1.0 - MIN_RATIO + 1e-9);
}

#[test]
fn resize_without_a_matching_split_is_a_noop() {
    let m = three();
    let (m2, effects) = apply(m.clone(), Command::Resize(Direction::Right));
    assert_eq!(m2, m);
    assert_eq!(effects, vec![]);
}

#[test]
fn toggle_zoom_fills_screen_and_follows_focus() {
    let (m, _) = apply(h_split(false), Command::ToggleZoom);
    assert_eq!(m.placements(), vec![(win(1), SCREEN)]);
    // Focus the right pane, still zoomed: that window now fills the screen.
    let (m, _) = apply(m, Command::Focus(Direction::Right));
    assert_eq!(m.placements(), vec![(win(2), SCREEN)]);
    let (m, _) = apply(m, Command::ToggleZoom);
    assert_eq!(
        m.placements(),
        vec![
            (win(1), Rect::new(0.0, 0.0, 50.0, 100.0)),
            (win(2), Rect::new(50.0, 0.0, 50.0, 100.0)),
        ]
    );
}

#[test]
fn remove_window_collapses_a_split_and_keeps_the_survivor() {
    let m = h_split(false);
    let (m, effects) = apply(m, Command::RemoveWindow(win(1)));
    assert_eq!(m.screens[0].tabs[0].root, Node::Leaf { id: PaneId(1), pane: Pane::Window(win(2)) });
    // Focus was on the removed pane → moves to the survivor.
    assert_eq!(m.focused_window(), Some(win(2)));
    assert!(effects.contains(&Effect::FocusWindow(win(2))));
}

#[test]
fn remove_window_drops_an_emptied_tab_and_clamps_current() {
    let m = three();
    let (m, _) = apply(m, Command::SelectTab(2)); // current = 2, window 3
    let (m, effects) = apply(m, Command::RemoveWindow(win(3)));
    assert_eq!(m.screens[0].tabs.len(), 2);
    assert_eq!(m.screens[0].current, 1);
    assert_eq!(m.focused_window(), Some(win(2)));
    assert!(effects.contains(&Effect::FocusWindow(win(2))));
}

#[test]
fn remove_an_unknown_window_is_a_noop() {
    let m = three();
    let (m2, effects) = apply(m.clone(), Command::RemoveWindow(win(99)));
    assert_eq!(m2, m);
    assert_eq!(effects, vec![]);
}

#[test]
fn sync_focus_locates_the_tab_and_pane_across_tabs() {
    let (m, effects) = apply(three(), Command::SyncFocus(win(3)));
    assert_eq!(m.screens[0].current, 2);
    assert_eq!(m.focused_window(), Some(win(3)));
    assert_eq!(effects, vec![]);
}

#[test]
fn raise_locates_across_tabs_and_focuses() {
    let (m, effects) = apply(three(), Command::Raise(win(2)));
    assert_eq!(m.screens[0].current, 1);
    assert_eq!(m.focused_window(), Some(win(2)));
    assert!(matches!(effects.first(), Some(Effect::Render(_))));
    assert!(effects.contains(&Effect::FocusWindow(win(2))));
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
fn raise_a_gone_window_is_a_noop() {
    let m = three();
    let (m2, effects) = apply(m.clone(), Command::Raise(win(99)));
    assert_eq!(m2, m);
    assert_eq!(effects, vec![]);
}

#[test]
fn fill_pane_moves_a_window_from_another_tab_into_the_empty_pane() {
    let (m, _) = apply(one(&[win(1), win(2)]), Command::Split(Dir::Horizontal));
    // Tab 0 is now [win1 | empty], empty focused; win2 lives in tab 1.
    let (m, effects) = apply(m, Command::FillPane(win(2)));
    assert_eq!(m.screens[0].tabs.len(), 1);
    assert_eq!(m.focused_window(), Some(win(2)));
    assert_eq!(
        m.placements(),
        vec![
            (win(1), Rect::new(0.0, 0.0, 50.0, 100.0)),
            (win(2), Rect::new(50.0, 0.0, 50.0, 100.0)),
        ]
    );
    assert!(effects.contains(&Effect::FocusWindow(win(2))));
}

#[test]
fn fill_pane_is_noop_when_focused_pane_is_not_empty() {
    let m = three(); // focused pane holds win1
    let (m2, effects) = apply(m.clone(), Command::FillPane(win(2)));
    assert_eq!(m2, m);
    assert_eq!(effects, vec![]);
}

#[test]
fn fill_pane_moves_a_same_tab_window_from_another_pane() {
    // Tab 0 = [win1 | empty(focused)]; win1 is in the split's other pane.
    let (m, _) = apply(one(&[win(1), win(2)]), Command::Split(Dir::Horizontal));
    let (m, _) = apply(m, Command::FillPane(win(1)));
    // win1 moves into the empty pane; the split collapses to just win1.
    assert_eq!(m.screens[0].tabs[0].root, Node::Leaf { id: PaneId(2), pane: Pane::Window(win(1)) });
    assert_eq!(m.focused_window(), Some(win(1)));
    assert_eq!(m.screens[0].tabs.len(), 2); // tab 1 (win2) untouched
}

#[test]
fn fill_pane_is_noop_for_a_window_in_the_focused_pane() {
    // Tab 0 = [win1 | empty(focused)]; filling with a window already in the
    // focused (empty) pane's node is impossible, but the guard also rejects any
    // window that resolves to the focused pane. A window in ANOTHER pane moves
    // (see above); one nowhere near is fine too — here we just assert the empty
    // pane can't be "filled" by nothing.
    let (m, _) = apply(one(&[win(1)]), Command::Stackify); // [win1, empty] stack, empty selected
    let before = m.clone();
    // win1 is the stack's other item → in the focused pane's node → no-op.
    let (m2, effects) = apply(m, Command::FillPane(win(1)));
    assert_eq!(m2, before);
    assert_eq!(effects, vec![]);
}

#[test]
fn fill_pane_decrements_current_when_an_earlier_tab_is_dropped() {
    let m = three(); // tabs [win1, win2, win3]
    let (m, _) = apply(m, Command::SelectTab(2)); // current = 2 (win3)
    let (m, _) = apply(m, Command::Split(Dir::Horizontal)); // tab 2 = [win3 | empty]
    let (m, _) = apply(m, Command::FillPane(win(1))); // pulls win1 from tab 0
    // Tab 0 dropped → current shifts 2 → 1, still the tab we split.
    assert_eq!(m.screens[0].tabs.len(), 2);
    assert_eq!(m.screens[0].current, 1);
    assert_eq!(m.focused_window(), Some(win(1)));
    assert_eq!(windows(&m.screens[0].tabs[1].root), vec![win(3), win(1)]);
}

#[test]
fn close_focused_pane_collapses_the_split() {
    let (m, _) = apply(one(&[win(1), win(2)]), Command::Split(Dir::Horizontal));
    // Tab 0 = [win1 | empty], empty focused.
    let (m, effects) = apply(m, Command::CloseFocusedPane);
    assert_eq!(m.screens[0].tabs[0].root, Node::Leaf { id: PaneId(0), pane: Pane::Window(win(1)) });
    assert_eq!(m.focused_window(), Some(win(1)));
    assert_eq!(m.placements(), vec![(win(1), SCREEN)]);
    assert!(effects.contains(&Effect::FocusWindow(win(1))));
}

#[test]
fn close_focused_pane_is_noop_on_a_window_pane() {
    let m = three(); // focused pane holds win1
    let (m2, effects) = apply(m.clone(), Command::CloseFocusedPane);
    assert_eq!(m2, m);
    assert_eq!(effects, vec![]);
}

#[test]
fn break_pane_pops_the_window_to_its_own_tab_and_collapses_the_split() {
    let (m, _) = apply(one(&[win(1), win(2)]), Command::Split(Dir::Horizontal));
    let (m, _) = apply(m, Command::FillPane(win(2))); // tab 0 = [win1 | win2], win2 focused
    assert_eq!(m.screens[0].tabs.len(), 1);
    let (m, effects) = apply(m, Command::BreakPane);
    // Current tab collapses back to win1; win2 becomes its own new tab.
    assert_eq!(m.screens[0].tabs.len(), 2);
    assert_eq!(m.screens[0].current, 0);
    assert_eq!(m.focused_window(), Some(win(1)));
    assert_eq!(windows(&m.screens[0].tabs[0].root), vec![win(1)]);
    assert_eq!(windows(&m.screens[0].tabs[1].root), vec![win(2)]);
    assert!(effects.contains(&Effect::FocusWindow(win(1))));
}

#[test]
fn break_pane_is_noop_on_a_single_pane_tab() {
    let m = three(); // each tab is a single window pane
    let (m2, effects) = apply(m.clone(), Command::BreakPane);
    assert_eq!(m2, m);
    assert_eq!(effects, vec![]);
}

#[test]
fn bar_tabs_reports_a_representative_window_per_tab() {
    let (m, _) = apply(three(), Command::Split(Dir::Horizontal));
    // Tab 0 now has an empty focused pane; its representative is window 1.
    let (tabs, current) = m.bar_tabs();
    let reps: Vec<_> = tabs.iter().map(|(_, r, _)| *r).collect();
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
    let (m, _) = apply(two_screens(), Command::ToggleZoom);
    let p = m.placements();
    // Focused screen 0's window fills its rect; screen 1 still tiles normally.
    assert!(p.contains(&(win(1), Rect::new(0.0, 0.0, 100.0, 100.0))));
    assert!(p.contains(&(win(2), Rect::new(100.0, 0.0, 100.0, 100.0))));
}

// --- Nested stacks -------------------------------------------------------

use crate::geometry::STACK_BAR_H;

fn stack(items: Vec<Pane>, selected: usize) -> Node {
    Node::Stack { id: PaneId(0), items, selected }
}

#[test]
fn stackify_wraps_a_window_leaf_with_an_empty_selected() {
    let (m, _) = apply(one(&[win(1)]), Command::Stackify);
    assert_eq!(m.screens[0].tabs[0].root, stack(vec![Pane::Window(win(1)), Pane::Empty], 1));
    assert_eq!(m.screens[0].tabs[0].focused, PaneId(0));
    assert!(m.focused_pane_is_empty());
}

#[test]
fn stackify_a_stack_pushes_another_empty() {
    let (m, _) = apply(one(&[win(1)]), Command::Stackify);
    let (m, _) = apply(m, Command::Stackify);
    assert_eq!(
        m.screens[0].tabs[0].root,
        stack(vec![Pane::Window(win(1)), Pane::Empty, Pane::Empty], 2)
    );
}

#[test]
fn stack_cycle_wraps() {
    let (m, _) = apply(one(&[win(1)]), Command::Stackify); // [W, E] selected=1
    let (m, _) = apply(m, Command::StackCycle(1)); // -> 0
    assert_eq!(m.screens[0].tabs[0].root, stack(vec![Pane::Window(win(1)), Pane::Empty], 0));
    let (m, _) = apply(m, Command::StackCycle(1)); // wraps -> 1
    assert_eq!(m.screens[0].tabs[0].root, stack(vec![Pane::Window(win(1)), Pane::Empty], 1));
}

#[test]
fn fill_pane_sets_the_selected_stack_item() {
    let (m, _) = apply(one(&[win(1), win(2)]), Command::Stackify); // tab0 -> [W1, E] sel 1
    let (m, _) = apply(m, Command::FillPane(win(2)));
    assert_eq!(m.screens[0].tabs.len(), 1); // win(2)'s old tab emptied and dropped
    assert_eq!(
        m.screens[0].tabs[0].root,
        stack(vec![Pane::Window(win(1)), Pane::Window(win(2))], 1)
    );
    assert_eq!(m.focused_window(), Some(win(2)));
}

#[test]
fn closing_a_stack_down_to_one_collapses_to_a_leaf() {
    let (m, _) = apply(one(&[win(1)]), Command::Stackify); // [W1, E] sel 1 (empty selected)
    let (m, _) = apply(m, Command::CloseFocusedPane);
    assert_eq!(m.screens[0].tabs[0].root, Node::Leaf { id: PaneId(0), pane: Pane::Window(win(1)) });
}

#[test]
fn raise_reveals_a_hidden_stack_member() {
    let (m, _) = apply(one(&[win(1), win(2)]), Command::Stackify);
    let (m, _) = apply(m, Command::FillPane(win(2))); // [W1, W2] sel 1 (W2 shown)
    let (m, _) = apply(m, Command::Raise(win(1))); // raising the hidden member
    assert_eq!(m.screens[0].tabs[0].root, stack(vec![Pane::Window(win(1)), Pane::Window(win(2))], 0));
    assert_eq!(m.focused_window(), Some(win(1)));
}

#[test]
fn stack_select_and_move_and_name() {
    let (m, _) = apply(one(&[win(1), win(2)]), Command::Stackify);
    let (m, _) = apply(m, Command::FillPane(win(2))); // [W1, W2] sel 1
    // Select the 1st window item.
    let (m, _) = apply(m, Command::StackSelect(1));
    assert!(matches!(m.screens[0].tabs[0].root, Node::Stack { selected: 0, .. }));
    // Name the selected item, then reorder it down.
    let (m, _) = apply(m, Command::SetStackName(Some("editor".into())));
    assert_eq!(m.stack_names.get(&win(1)), Some(&"editor".to_string()));
    let (m, _) = apply(m, Command::StackMove(1)); // W1 moves to index 1, stays selected
    assert_eq!(m.screens[0].tabs[0].root, stack(vec![Pane::Window(win(2)), Pane::Window(win(1))], 1));
}

#[test]
fn break_pane_from_a_stack_in_a_split_keeps_the_rest_in_the_split() {
    use std::collections::HashMap;
    // A split whose right pane is a 2-window stack (win 3 selected).
    let root = Node::Split {
        dir: Dir::Horizontal,
        ratios: vec![0.5, 0.5],
        children: vec![
            Node::Leaf { id: PaneId(1), pane: Pane::Window(win(1)) },
            Node::Stack {
                id: PaneId(2),
                items: vec![Pane::Window(win(2)), Pane::Window(win(3))],
                selected: 1,
            },
        ],
    };
    let m = Model {
        screens: vec![Screen {
            rect: SCREEN,
            tabs: vec![Tab { root, focused: PaneId(2), name: None }],
            current: 0,
        }],
        focused_screen: 0,
        zoomed: false,
        stack_names: HashMap::new(),
        next_pane_id: 3,
    };
    let (m, _) = apply(m, Command::BreakPane);
    assert_eq!(m.screens[0].tabs.len(), 2);
    // Only the selected window (win 3) pops out to its own tab.
    assert_eq!(m.screens[0].tabs[1].root, Node::Leaf { id: PaneId(3), pane: Pane::Window(win(3)) });
    // The other stack window (win 2) stays in the split, now a plain leaf.
    assert_eq!(
        m.screens[0].tabs[0].root,
        Node::Split {
            dir: Dir::Horizontal,
            ratios: vec![0.5, 0.5],
            children: vec![
                Node::Leaf { id: PaneId(1), pane: Pane::Window(win(1)) },
                Node::Leaf { id: PaneId(2), pane: Pane::Window(win(2)) },
            ],
        }
    );
}

#[test]
fn break_pane_from_a_plain_stack_tab_pops_the_item_and_keeps_its_name() {
    use std::collections::HashMap;
    // A whole-tab stack of two windows, win 2 selected and named "editor".
    let mut stack_names = HashMap::new();
    stack_names.insert(win(2), "editor".to_string());
    let m = Model {
        screens: vec![Screen {
            rect: SCREEN,
            tabs: vec![Tab {
                root: Node::Stack {
                    id: PaneId(0),
                    items: vec![Pane::Window(win(1)), Pane::Window(win(2))],
                    selected: 1,
                },
                focused: PaneId(0),
                name: None,
            }],
            current: 0,
        }],
        focused_screen: 0,
        zoomed: false,
        stack_names,
        next_pane_id: 1,
    };
    let (m, _) = apply(m, Command::BreakPane);
    assert_eq!(m.screens[0].tabs.len(), 2);
    // The stack collapses to its remaining window (win 1).
    assert_eq!(m.screens[0].tabs[0].root, Node::Leaf { id: PaneId(0), pane: Pane::Window(win(1)) });
    // win 2 pops to its own tab and keeps the name it had in the stack.
    assert_eq!(m.screens[0].tabs[1].root, Node::Leaf { id: PaneId(1), pane: Pane::Window(win(2)) });
    assert_eq!(m.screens[0].tabs[1].name.as_deref(), Some("editor"));
    assert!(m.stack_names.is_empty()); // the name moved to the tab
}

#[test]
fn cancel_stackify_on_a_split_pane_keeps_focus_on_that_pane() {
    use std::collections::HashMap;
    // Split with win2 (the second pane) focused.
    let root = Node::Split {
        dir: Dir::Horizontal,
        ratios: vec![0.5, 0.5],
        children: vec![
            Node::Leaf { id: PaneId(1), pane: Pane::Window(win(1)) },
            Node::Leaf { id: PaneId(2), pane: Pane::Window(win(2)) },
        ],
    };
    let m = Model {
        screens: vec![Screen {
            rect: SCREEN,
            tabs: vec![Tab { root, focused: PaneId(2), name: None }],
            current: 0,
        }],
        focused_screen: 0,
        zoomed: false,
        stack_names: HashMap::new(),
        next_pane_id: 3,
    };
    let (m, _) = apply(m, Command::Stackify); // win2 → stack with an empty slot
    assert!(m.focused_pane_is_empty());
    let (m, _) = apply(m, Command::CloseFocusedPane); // cancel the stackify
    // The stack collapses back to win2 in place; focus stays on it, not win1.
    assert_eq!(m.focused_window(), Some(win(2)));
    assert_eq!(
        m.screens[0].tabs[0].root,
        Node::Split {
            dir: Dir::Horizontal,
            ratios: vec![0.5, 0.5],
            children: vec![
                Node::Leaf { id: PaneId(1), pane: Pane::Window(win(1)) },
                Node::Leaf { id: PaneId(2), pane: Pane::Window(win(2)) },
            ],
        }
    );
}

#[test]
fn placements_place_only_the_selected_stack_item() {
    let (m, _) = apply(one(&[win(1), win(2)]), Command::Stackify);
    let (m, _) = apply(m, Command::FillPane(win(2))); // stack [W1, W2] sel 1
    let content = Rect::new(0.0, STACK_BAR_H, 100.0, 100.0 - STACK_BAR_H);
    assert_eq!(m.placements(), vec![(win(2), content)]);
}

#[test]
fn stacks_reports_the_stack_rect_and_items() {
    let (m, _) = apply(one(&[win(1), win(2)]), Command::Stackify);
    let (m, _) = apply(m, Command::FillPane(win(2))); // stack [W1, W2] sel 1
    assert_eq!(
        m.stacks(),
        vec![StackBar { rect: SCREEN, items: vec![win(1), win(2)], selected: 1, focused: true }]
    );
}
