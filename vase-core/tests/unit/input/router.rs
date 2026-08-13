use super::*;
use std::collections::HashMap;

fn router() -> KeyRouter {
    let mut b = HashMap::new();
    b.insert(Key::plain(KeyCode::Right), InputCommand::FocusRight);
    KeyRouter::new(Key::meta(KeyCode::Char('a')), b)
}

#[test]
fn idle_passes_non_prefix_through() {
    let mut r = router();
    assert_eq!(r.key(Key::plain(KeyCode::Right)), Decision::PassThrough);
    assert!(!r.is_armed());
}

#[test]
fn prefix_chord_arms_and_is_consumed() {
    let mut r = router();
    assert_eq!(r.key(Key::meta(KeyCode::Char('a'))), Decision::Consume);
    assert!(r.is_armed());
}

#[test]
fn armed_bound_key_runs_and_disarms() {
    let mut r = router();
    r.key(Key::meta(KeyCode::Char('a')));
    assert_eq!(r.key(Key::plain(KeyCode::Right)), Decision::ConsumeAndRun(InputCommand::FocusRight));
    assert!(!r.is_armed());
}

#[test]
fn armed_unbound_key_passes_through_and_disarms() {
    let mut r = router();
    r.key(Key::meta(KeyCode::Char('a')));
    assert_eq!(r.key(Key::plain(KeyCode::Char('/'))), Decision::PassThrough); // unbound
    assert!(!r.is_armed(), "must disarm even on a miss");
}

#[test]
fn plain_a_without_command_is_not_the_prefix() {
    let mut r = router();
    assert_eq!(r.key(Key::plain(KeyCode::Char('a'))), Decision::PassThrough); // 'a' alone
    assert!(!r.is_armed());
}

#[test]
fn a_second_prefix_gets_its_own_bindings() {
    let mut nested = HashMap::new();
    nested.insert(Key::plain(KeyCode::Right), InputCommand::StackFocusNext);
    let mut r = router().with_prefix(Key::meta(KeyCode::Char('e')), nested);
    r.key(Key::meta(KeyCode::Char('e')));
    assert_eq!(r.key(Key::plain(KeyCode::Right)), Decision::ConsumeAndRun(InputCommand::StackFocusNext));
    // The original prefix still routes to the original set.
    r.key(Key::meta(KeyCode::Char('a')));
    assert_eq!(r.key(Key::plain(KeyCode::Right)), Decision::ConsumeAndRun(InputCommand::FocusRight));
}
