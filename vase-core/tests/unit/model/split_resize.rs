use super::*;

fn split_ratios(node: &Node) -> Vec<f64> {
    match node {
        Node::Split { ratios, .. } => ratios.clone(),
        _ => panic!("not a split"),
    }
}

#[test]
fn split_makes_an_empty_focused_pane_and_reflows_the_window() {
    let (m, effects) = apply(three(), Command::Split(Dir::Horizontal));
    // The new empty pane is focused (a fresh PaneId), no window under focus.
    assert_eq!(m.focused_window(), None);
    assert_eq!(
        m.screens[0].tabs[0].root,
        Node::Split { dir: Dir::Horizontal, ratios: vec![0.5, 0.5], children: vec![Node::Leaf { id: PaneId(0), pane: Pane::Window(win(1)) }, Node::Leaf { id: PaneId(3), pane: Pane::Empty },] }
    );
    assert_eq!(m.screens[0].tabs[0].focused, PaneId(3));
    // Only the window reflows into its half; NO FocusWindow (empty focus).
    assert_eq!(effects, vec![Effect::Render(vec![(win(1), Rect::new(0.0, 0.0, 50.0, 100.0))])]);
    assert_eq!(m.empty_panes(), vec![(Rect::new(50.0, 0.0, 50.0, 100.0), true)]);
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
    assert_eq!(m.placements(), vec![(win(1), Rect::new(0.0, 0.0, 50.0, 100.0)), (win(2), Rect::new(50.0, 0.0, 50.0, 100.0)),]);
}
