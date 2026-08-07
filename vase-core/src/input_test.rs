use super::*;
use crate::tree::WindowId;

fn router() -> KeyRouter {
    let mut b = HashMap::new();
    b.insert(Key::plain(0x7C), InputCommand::FocusRight); // Right arrow
    KeyRouter::new(Key::cmd(0x00), b) // prefix = Cmd-a (keycode 0x00 = 'a')
}

#[test]
fn idle_passes_non_prefix_through() {
    let mut r = router();
    assert_eq!(r.key(Key::plain(0x7C)), Decision::PassThrough);
    assert!(!r.is_armed());
}

#[test]
fn prefix_chord_arms_and_is_consumed() {
    let mut r = router();
    assert_eq!(r.key(Key::cmd(0x00)), Decision::Consume);
    assert!(r.is_armed());
}

#[test]
fn armed_bound_key_runs_and_disarms() {
    let mut r = router();
    r.key(Key::cmd(0x00));
    assert_eq!(r.key(Key::plain(0x7C)), Decision::ConsumeAndRun(InputCommand::FocusRight));
    assert!(!r.is_armed());
}

#[test]
fn armed_unbound_key_passes_through_and_disarms() {
    let mut r = router();
    r.key(Key::cmd(0x00));
    assert_eq!(r.key(Key::plain(0x2C)), Decision::PassThrough); // unbound
    assert!(!r.is_armed(), "must disarm even on a miss");
}

#[test]
fn plain_a_without_command_is_not_the_prefix() {
    let mut r = router();
    assert_eq!(r.key(Key::plain(0x00)), Decision::PassThrough); // 'a' alone
    assert!(!r.is_armed());
}

fn sw() -> Switcher<WindowId> {
    Switcher::new(vec![
        (WindowId(1), "Ghostty".into()),
        (WindowId(2), "Google Chrome".into()),
        (WindowId(3), "Obsidian".into()),
    ])
}

#[test]
fn move_down_and_select() {
    let mut s = sw();
    s.move_down();
    assert_eq!(s.selection(), Some(WindowId(2)));
}

#[test]
fn move_up_from_top_wraps_to_bottom() {
    let mut s = sw(); // 3 items, selected at top
    s.move_up();
    assert_eq!(s.selection(), Some(WindowId(3)));
    // And down from the bottom wraps back to the top.
    s.move_down();
    assert_eq!(s.selection(), Some(WindowId(1)));
}

#[test]
fn typing_filters_case_insensitively() {
    let mut s = sw();
    s.type_char('c');
    s.type_char('H');
    assert_eq!(s.visible().iter().map(|(id, _)| *id).collect::<Vec<_>>(), vec![WindowId(2)]);
    assert_eq!(s.selection(), Some(WindowId(2)));
}

#[test]
fn filtering_clamps_the_selection() {
    let mut s = sw();
    s.move_down();
    s.move_down();
    s.type_char('g');
    assert!(s.selected() <= 1);
    s.backspace();
    assert_eq!(s.visible().len(), 3);
}

#[test]
fn move_top_and_bottom() {
    let mut s = sw();
    s.move_bottom();
    assert_eq!(s.selection(), Some(WindowId(3)));
    s.move_top();
    assert_eq!(s.selection(), Some(WindowId(1)));
}

#[test]
fn search_mode_toggles_and_clears() {
    let mut s = sw();
    assert!(!s.is_searching());
    s.start_search();
    assert!(s.is_searching());
    s.type_char('g'); // Ghostty + Google Chrome contain 'g', Obsidian doesn't
    assert_eq!(s.visible().len(), 2);
    s.stop_search();
    assert!(!s.is_searching());
    assert_eq!(s.query(), "");
    assert_eq!(s.visible().len(), 3); // filter cleared
}
