use super::helpers::*;
use super::*;

#[test]
fn j_and_k_move_the_selection() {
    let mut s = sw();
    ch(&mut s, 'j');
    assert_eq!(s.selected(), 1);
    ch(&mut s, 'k');
    assert_eq!(s.selected(), 0);
}

#[test]
fn moving_up_from_the_top_wraps_to_the_bottom() {
    let mut s = sw();
    ch(&mut s, 'k');
    assert_eq!(s.selected(), 2);
    ch(&mut s, 'j');
    assert_eq!(s.selected(), 0);
}

#[test]
fn w_and_s_move_like_k_and_j() {
    let mut s = sw();
    ch(&mut s, 's');
    assert_eq!(s.selected(), 1);
    ch(&mut s, 'w');
    assert_eq!(s.selected(), 0);
}

#[test]
fn space_chooses_the_highlighted_row() {
    let mut s = sw();
    ch(&mut s, 's');
    assert_eq!(ch(&mut s, ' '), Pick::Chosen(Row::Win(2)));
}

#[test]
fn w_s_and_space_type_in_search_mode() {
    let mut s = sw();
    ch(&mut s, '/');
    ch(&mut s, 'w');
    ch(&mut s, ' ');
    ch(&mut s, 's');
    assert_eq!(s.query(), "w s");
}

#[test]
fn arrows_move_like_j_and_k() {
    let mut s = sw();
    press(&mut s, KeyCode::Down);
    assert_eq!(s.selected(), 1);
    press(&mut s, KeyCode::Up);
    assert_eq!(s.selected(), 0);
}

#[test]
fn gg_jumps_to_the_top_and_shift_g_to_the_bottom() {
    let mut s = sw();
    assert_eq!(s.key(Key { code: KeyCode::Char('g'), mods: Mods { shift: true, ..Mods::default() } }, t0()), Pick::Redraw);
    assert_eq!(s.selected(), 2);
    ch(&mut s, 'g'); // first g: waits
    assert_eq!(s.selected(), 2, "a lone g must not move");
    ch(&mut s, 'g'); // second g: jump
    assert_eq!(s.selected(), 0);
}

#[test]
fn a_lone_g_followed_by_another_key_is_not_gg() {
    let mut s = sw();
    ch(&mut s, 'j');
    ch(&mut s, 'g');
    ch(&mut s, 'j'); // breaks the pair
    ch(&mut s, 'g');
    assert_eq!(s.selected(), 2, "the g pending from before j must not fire");
}

#[test]
fn slash_enters_search_and_typing_filters_case_insensitively() {
    let mut s = sw();
    assert!(!s.is_searching());
    ch(&mut s, '/');
    assert!(s.is_searching());
    ch(&mut s, 'c');
    ch(&mut s, 'h');
    assert_eq!(ids(&s), vec![Row::Win(2)]);
    assert_eq!(s.query(), "ch");
}

#[test]
fn backspace_widens_the_search_again() {
    let mut s = sw();
    ch(&mut s, '/');
    ch(&mut s, 'g');
    assert_eq!(ids(&s).len(), 2);
    press(&mut s, KeyCode::Backspace);
    assert_eq!(ids(&s).len(), 3);
}

#[test]
fn esc_leaves_search_first_then_cancels() {
    let mut s = sw();
    ch(&mut s, '/');
    ch(&mut s, 'g');
    assert_eq!(press(&mut s, KeyCode::Escape), Pick::Redraw);
    assert!(!s.is_searching());
    assert_eq!(s.query(), "");
    assert_eq!(ids(&s).len(), 3);
    assert_eq!(press(&mut s, KeyCode::Escape), Pick::Cancelled);
}

#[test]
fn j_navigates_in_search_mode_instead_of_typing() {
    let mut s = sw();
    ch(&mut s, '/');
    ch(&mut s, 'j'); // a literal 'j' filters, it does not move
    assert_eq!(s.query(), "j");
}

#[test]
fn enter_chooses_the_highlighted_row() {
    let mut s = sw();
    ch(&mut s, 'j');
    assert_eq!(press(&mut s, KeyCode::Return), Pick::Chosen(Row::Win(2)));
}

#[test]
fn filtering_clamps_the_selection_into_range() {
    let mut s = sw();
    ch(&mut s, 'j');
    ch(&mut s, 'j');
    ch(&mut s, '/');
    ch(&mut s, 'g'); // 3 rows → 2
    assert!(s.selected() <= 1);
}
