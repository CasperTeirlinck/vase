use super::*;
use crate::chrome::deck::{route_click, BarMap, Click, ClickMap};
use crate::chrome::BarHits;
use crate::geometry::Rect;
use crate::model::{Command, Model};
use crate::tree::WindowId;

/// A bar 200 wide at y=780, one 50-wide range per tab.
fn bar() -> BarMap {
    BarMap { rect: Rect::new(0.0, 780.0, 200.0, bar_height()), hits: BarHits { tabs: vec![(0.0, 50.0), (50.0, 100.0), (100.0, 150.0), (150.0, 200.0)], apps: Vec::new() }, apps: Vec::new() }
}

/// The same bar with two trailing windowless-app icons past the last tab.
fn bar_with_apps() -> BarMap {
    BarMap {
        rect: Rect::new(0.0, 780.0, 300.0, bar_height()),
        hits: BarHits { tabs: bar().hits.tabs, apps: vec![(220.0, 240.0), (245.0, 265.0)] },
        apps: vec!["Notes".to_string(), "Music".to_string()],
    }
}

/// A stack bar sitting inside the content area, two items.
fn stack() -> ClickMap {
    (Rect::new(300.0, 100.0, 200.0, bar_height()), vec![(0.0, 100.0), (100.0, 200.0)], vec![WindowId(7), WindowId(8)])
}

/// Two screens: 2 tabs on the first, 2 on the second.
fn model() -> Model {
    Model::adopt(&[Rect::new(0.0, 0.0, 1000.0, 780.0), Rect::new(1000.0, 0.0, 1000.0, 800.0)], &[(WindowId(1), 0), (WindowId(2), 0), (WindowId(3), 1), (WindowId(4), 1)])
}

fn click(px: f64, py: f64) -> Option<Command> {
    match route_click(&model(), Some(&bar()), &[stack()], px, py) {
        Some(Click::Command(cmd)) => Some(cmd),
        Some(Click::Activate(app)) => panic!("expected a command, got {app}"),
        None => None,
    }
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
    let overlapping = (Rect::new(0.0, 780.0, 200.0, bar_height()), vec![(0.0, 200.0)], vec![WindowId(9)]);
    assert_eq!(route_click(&model(), Some(&bar()), &[overlapping], 30.0, 785.0), Some(Click::Command(Command::SelectStackWindow(WindowId(9)))));
}

#[test]
fn with_no_bar_drawn_nothing_is_ours() {
    assert_eq!(route_click(&model(), None, &[], 30.0, 785.0), None);
}

#[test]
fn a_stale_range_past_the_end_of_the_tabs_selects_nothing() {
    // Ranges outlive a model that lost tabs; a hit past the end must not panic or pick wrongly.
    let five = (Rect::new(0.0, 780.0, 500.0, bar_height()), vec![(0.0, 100.0); 1]);
    let mut ranges = five.1.clone();

    ranges.extend([(100.0, 200.0), (200.0, 300.0), (300.0, 400.0), (400.0, 500.0)]);
    let wide = BarMap { rect: five.0, hits: BarHits { tabs: ranges, apps: Vec::new() }, apps: Vec::new() };
    assert_eq!(route_click(&model(), Some(&wide), &[], 450.0, 785.0), None, "5th range, only 4 tabs");
}

#[test]
fn a_click_on_a_trailing_icon_activates_that_app() {
    let map = bar_with_apps();
    let hit = |px: f64| route_click(&model(), Some(&map), &[], px, 785.0);
    assert_eq!(hit(230.0), Some(Click::Activate("Notes".to_string())));
    assert_eq!(hit(250.0), Some(Click::Activate("Music".to_string())));
    // The gap between two icons, and the run between the last tab and the first icon, are nobody's.
    assert_eq!(hit(242.0), None);
    assert_eq!(hit(210.0), None);
    // Tabs still win inside their own ranges.
    assert_eq!(hit(30.0), Some(Click::Command(Command::SelectScreenTab(0, 0))));
}
