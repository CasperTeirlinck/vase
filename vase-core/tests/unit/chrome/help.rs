use crate::chrome::help::{layout, sections, TextStyle};
use crate::geometry::Rect;
use crate::input::keymap;

fn screen() -> Rect {
    Rect::new(0.0, 0.0, 2560.0, 1440.0)
}

#[test]
fn every_row_shows_a_chord_the_keymap_really_binds() {
    let (top, nested) = (keymap::bindings(), keymap::bindings_nested());
    let mut checked = 0;
    for section in sections() {
        for row in section.rows {
            let Some((key, cmd, in_stack_mode)) = row.binding else { continue };
            let map = if in_stack_mode { &nested } else { &top };
            assert_eq!(map.get(&key), Some(&cmd), "{:?} is on the sheet as {:?} but the keymap disagrees", row.keys, cmd);
            checked += 1;
        }
    }
    assert!(checked > 20, "the sheet should cover the keymap, not a corner of it");
}

#[test]
fn the_card_is_centered_and_holds_everything_it_draws() {
    let l = layout(screen());
    // Centered on the screen it was laid out for.
    assert_eq!(l.rect.x + l.rect.w / 2.0, screen().w / 2.0);
    assert_eq!(l.rect.y + l.rect.h / 2.0, screen().h / 2.0);

    let inside = |r: Rect| r.x >= 0.0 && r.y >= 0.0 && r.x + r.w <= l.rect.w && r.y + r.h <= l.rect.h;
    assert!(l.texts.iter().all(|t| inside(t.rect)), "a line of the sheet spills out of the card");
    assert!(l.cells.iter().all(|c| inside(c.rect)), "a miniature spills out of the card");

    // Two columns, each running top to bottom: within one column every row sits below the last.
    let mut columns: Vec<Vec<f64>> = vec![Vec::new(), Vec::new()];
    for text in l.texts.iter().filter(|t| t.style == TextStyle::Label) {
        columns[usize::from(text.rect.x > l.rect.w / 2.0)].push(text.rect.y);
    }
    assert!(columns.iter().all(|c| c.len() > 4), "both columns carry rows");
    assert!(columns.iter().all(|c| c.windows(2).all(|w| w[0] < w[1])), "rows must not overlap or backtrack");
}
