use super::helpers::t0;
use super::*;

#[test]
fn single_digit_commits_at_once_when_no_larger_number_fits() {
    let mut e = NumberEntry::default();
    assert_eq!(e.digit(3, 5, t0()), Entry::Commit(3)); // 30 > 5, nothing longer possible
    assert!(!e.is_pending());
}

#[test]
fn a_digit_that_could_grow_waits_for_the_next_one() {
    let mut e = NumberEntry::default();
    assert_eq!(e.digit(1, 15, t0()), Entry::Pending); // 12, 13, … still reachable
    assert!(e.is_pending());
    assert_eq!(e.digit(2, 15, t0()), Entry::Commit(12));
}

#[test]
fn an_out_of_range_digit_is_dropped_and_commits_what_was_buffered() {
    let mut e = NumberEntry::default();
    e.digit(1, 15, t0());
    assert_eq!(e.digit(9, 15, t0()), Entry::Commit(1), "19 > 15, so 1 stands");
}

#[test]
fn zero_never_commits() {
    let mut e = NumberEntry::default();
    assert_eq!(e.digit(0, 9, t0()), Entry::Idle);
}

#[test]
fn a_half_typed_number_commits_once_its_deadline_passes() {
    let mut e = NumberEntry::default();
    let now = t0();
    e.digit(1, 15, now);
    assert_eq!(e.tick(now), Entry::Idle, "not yet");
    assert_eq!(e.tick(now + ENTRY_TIMEOUT), Entry::Commit(1));
    assert_eq!(e.tick(now + ENTRY_TIMEOUT), Entry::Idle, "commits once");
}

#[test]
fn cancel_discards_but_flush_commits() {
    let mut e = NumberEntry::default();
    e.digit(1, 15, t0());
    e.cancel();
    assert_eq!(e.flush(), Entry::Idle);

    e.digit(1, 15, t0());
    assert_eq!(e.flush(), Entry::Commit(1));
}
