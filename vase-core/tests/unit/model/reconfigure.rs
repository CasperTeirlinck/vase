use super::*;

#[test]
fn retain_windows_prunes_dead_windows_and_empty_tabs() {
    let mut m = two_screens();
    m.retain_windows(&HashSet::from([win(1)]));
    assert_eq!(windows(&m.screens[0].tabs[0].root), vec![win(1)]);
    assert!(m.screens[1].tabs.is_empty()); // win2's tab dropped
}

#[test]
fn retain_windows_collapses_a_split_around_a_dead_window() {
    let mut m = h_split(false);
    m.retain_windows(&HashSet::from([win(2)]));
    assert_eq!(m.screens[0].tabs.len(), 1);
    assert_eq!(windows(&m.screens[0].tabs[0].root), vec![win(2)]);
}

#[test]
fn reconfigure_shrinks_and_migrates_tabs_to_the_last_screen() {
    let m = &mut two_screens();
    let one_rect = [Rect { x: 0.0, y: 0.0, w: 100.0, h: 100.0 }];
    m.reconfigure(&one_rect);
    assert_eq!(m.screens.len(), 1);
    assert_eq!(windows(&m.screens[0].tabs[0].root), vec![win(1)]);
    assert_eq!(windows(&m.screens[0].tabs[1].root), vec![win(2)]);
}

#[test]
fn remap_windows_renames_survivors_and_drops_unmapped() {
    let mut m = two_screens();
    // win1 -> win10 (reboot reassigned its id); win2 has no live match.
    let map = HashMap::from([(win(1), win(10))]);
    m.remap_windows(&map);
    assert_eq!(windows(&m.screens[0].tabs[0].root), vec![win(10)]);
    assert!(m.screens[1].tabs.is_empty());
}

#[test]
fn reconfigure_grows_with_empty_screens_and_updates_rects() {
    let m = &mut one(&[win(1)]);
    let rects = [Rect { x: 0.0, y: 0.0, w: 100.0, h: 100.0 }, Rect { x: 100.0, y: 0.0, w: 200.0, h: 200.0 }];
    m.reconfigure(&rects);
    assert_eq!(m.screens.len(), 2);
    assert!(m.screens[1].tabs.is_empty());
    assert_eq!(m.screens[1].rect, rects[1]);
}
