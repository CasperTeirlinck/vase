use crate::chrome::*;

mod bar;
mod deck;
mod theme;

#[test]
fn the_bar_reserves_its_strip_on_the_edge_it_sits_on() {
    let work = Rect::new(0.0, 100.0, 2560.0, 1000.0);
    // Bottom: the content keeps the top edge. Top: it starts a strip lower.
    let bottom = usable(work, true, Position::Bottom);
    assert_eq!((bottom.y, bottom.h), (100.0, 1000.0 - BAR_HEIGHT));
    let top = usable(work, true, Position::Top);
    assert_eq!((top.y, top.h), (100.0 + BAR_HEIGHT, 1000.0 - BAR_HEIGHT));
    // Either way the strip lands inside the work area, and only the main display gives one up.
    assert_eq!(bottom.y + bottom.h + BAR_HEIGHT, work.y + work.h);
    assert_eq!(top.y - BAR_HEIGHT, work.y);
    assert_eq!(usable(work, false, Position::Top), work);
}

#[test]
fn scroll_offset_keeps_selection_in_view() {
    // Selection within the first window: no scroll.
    assert_eq!(scroll_offset(0, 5), 0);
    assert_eq!(scroll_offset(4, 5), 0);
    // Past the window: scroll so the selection is the last visible row.
    assert_eq!(scroll_offset(5, 5), 1);
    assert_eq!(scroll_offset(9, 5), 5);
    // Degenerate window.
    assert_eq!(scroll_offset(3, 0), 0);
}
