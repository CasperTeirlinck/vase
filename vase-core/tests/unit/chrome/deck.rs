use super::*;
use crate::chrome::deck::{route_click, ClickMap};
use crate::geometry::Rect;
use crate::model::{Command, Model};
use crate::tree::WindowId;

/// A bar 200 wide at y=780, one 50-wide range per tab.
fn bar() -> (Rect, Vec<(f64, f64)>) {
    (Rect::new(0.0, 780.0, 200.0, BAR_HEIGHT), vec![(0.0, 50.0), (50.0, 100.0), (100.0, 150.0), (150.0, 200.0)])
}

/// A stack bar sitting inside the content area, two items.
fn stack() -> ClickMap {
    (Rect::new(300.0, 100.0, 200.0, BAR_HEIGHT), vec![(0.0, 100.0), (100.0, 200.0)], vec![WindowId(7), WindowId(8)])
}

/// Two screens: 2 tabs on the first, 2 on the second.
fn model() -> Model {
    Model::adopt(&[Rect::new(0.0, 0.0, 1000.0, 780.0), Rect::new(1000.0, 0.0, 1000.0, 800.0)], &[(WindowId(1), 0), (WindowId(2), 0), (WindowId(3), 1), (WindowId(4), 1)])
}

fn click(px: f64, py: f64) -> Option<Command> {
    route_click(&model(), Some(&bar()), &[stack()], px, py)
}

#[test]
fn a_click_on_a_tab_selects_it() {
    assert_eq!(click(30.0, 785.0), Some(Command::SelectScreenTab(0, 0)));
    assert_eq!(click(70.0, 785.0), Some(Command::SelectScreenTab(0, 1)));
}

#[test]
fn flat_bar_order_carries_across_screens() {
    // Tabs 3 and 4 in bar order live on the second screen, at its own indices 0 and 1.
    assert_eq!(click(120.0, 785.0), Some(Command::SelectScreenTab(1, 0)));
    assert_eq!(click(170.0, 785.0), Some(Command::SelectScreenTab(1, 1)));
}

#[test]
fn range_edges_belong_to_the_tab_on_the_right() {
    assert_eq!(click(49.9, 785.0), Some(Command::SelectScreenTab(0, 0)));
    assert_eq!(click(50.0, 785.0), Some(Command::SelectScreenTab(0, 1)), "a range starts at its lower bound");
}

#[test]
fn a_click_outside_the_bar_is_not_ours() {
    assert_eq!(click(30.0, 400.0), None, "above the bar");
    assert_eq!(click(30.0, 779.9), None, "one point above the bar");
    assert_eq!(click(250.0, 785.0), None, "past the last tab");
    assert_eq!(click(-1.0, 785.0), None, "left of the bar");
}

#[test]
fn a_click_on_a_stack_bar_selects_that_stack_item() {
    assert_eq!(click(350.0, 105.0), Some(Command::SelectStackWindow(WindowId(7))));
    assert_eq!(click(450.0, 105.0), Some(Command::SelectStackWindow(WindowId(8))));
}

#[test]
fn a_stack_bar_wins_over_the_tab_bar_where_they_overlap() {
    // A stack bar drawn across the tab bar's strip takes the click.
    let overlapping = (Rect::new(0.0, 780.0, 200.0, BAR_HEIGHT), vec![(0.0, 200.0)], vec![WindowId(9)]);
    assert_eq!(route_click(&model(), Some(&bar()), &[overlapping], 30.0, 785.0), Some(Command::SelectStackWindow(WindowId(9))));
}

#[test]
fn with_no_bar_drawn_nothing_is_ours() {
    assert_eq!(route_click(&model(), None, &[], 30.0, 785.0), None);
}

#[test]
fn a_stale_range_past_the_end_of_the_tabs_selects_nothing() {
    // Ranges outlive a model that lost tabs; a hit past the end must not panic or pick wrongly.
    let five = (Rect::new(0.0, 780.0, 500.0, BAR_HEIGHT), vec![(0.0, 100.0); 1]);
    let mut ranges = five.1.clone();
    ranges.extend([(100.0, 200.0), (200.0, 300.0), (300.0, 400.0), (400.0, 500.0)]);
    let wide = (five.0, ranges);
    assert_eq!(route_click(&model(), Some(&wide), &[], 450.0, 785.0), None, "5th range, only 4 tabs");
}
