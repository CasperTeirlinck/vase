use super::helpers::*;
use super::*;

#[test]
fn a_digit_picks_a_row_by_its_number() {
    let mut s = sw();
    assert_eq!(press(&mut s, key_code_for_name("2").unwrap()), Pick::Chosen(Row::Win(2)));
}

#[test]
fn the_selection_never_rests_on_an_unselectable_row() {
    let mut s = Switcher::new(vec![(Row::Head(0), "header".into()), (Row::Win(1), "one".into()), (Row::Head(9), "header".into()), (Row::Win(2), "two".into())]);
    assert_eq!(s.selected(), 1, "a leading header is skipped on open");
    ch(&mut s, 'j');
    assert_eq!(s.selected(), 3, "j steps over the header");
    ch(&mut s, 'k');
    assert_eq!(s.selected(), 1, "k steps back over it");
}

#[test]
fn numbering_counts_only_numbered_rows() {
    let mut s = Switcher::new(vec![(Row::Head(0), "header".into()), (Row::Win(1), "one".into()), (Row::Win(2), "two".into()), (Row::Launch(7), "an app".into())]);
    // Row 2 is the second *numbered* row, not the second visible row.
    assert_eq!(press(&mut s, key_code_for_name("2").unwrap()), Pick::Chosen(Row::Win(2)));
}

#[test]
fn a_launch_row_is_selectable_but_unnumbered() {
    let mut s = Switcher::new(vec![(Row::Win(1), "one".into()), (Row::Launch(7), "an app".into())]);
    ch(&mut s, 'j');
    assert_eq!(press(&mut s, VK_RETURN), Pick::Chosen(Row::Launch(7)), "reachable by ⏎");
    // …but a digit only ever addresses the one numbered row.
    assert_eq!(press(&mut s, key_code_for_name("2").unwrap()), Pick::Ignored);
}

#[test]
fn a_half_typed_index_commits_on_tick() {
    let rows: Vec<(Row, String)> = (1..=12).map(|i| (Row::Win(i), format!("row {i}"))).collect();
    let mut s = Switcher::new(rows);
    let now = t0();
    assert_eq!(s.key(Key::plain(key_code_for_name("1").unwrap()), now), Pick::Ignored, "11, 12 still reachable");
    assert_eq!(s.tick(now + ENTRY_TIMEOUT), Pick::Chosen(Row::Win(1)));
}

#[test]
fn a_navigation_key_abandons_a_half_typed_index() {
    let rows: Vec<(Row, String)> = (1..=12).map(|i| (Row::Win(i), format!("row {i}"))).collect();
    let mut s = Switcher::new(rows);
    let now = t0();
    s.key(Key::plain(key_code_for_name("1").unwrap()), now);
    ch(&mut s, 'j');
    assert_eq!(s.tick(now + ENTRY_TIMEOUT), Pick::Ignored, "the 1 was dropped, not committed");
    assert_eq!(s.selected(), 1);
}

#[test]
fn an_unbound_key_is_ignored_rather_than_swallowed_silently() {
    let mut s = sw();
    assert_eq!(ch(&mut s, 'q'), Pick::Ignored);
    assert_eq!(s.selected(), 0);
}
