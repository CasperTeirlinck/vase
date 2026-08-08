use crate::overlay::text::*;

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
