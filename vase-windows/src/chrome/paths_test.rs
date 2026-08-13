use super::*;
use windows::Win32::Graphics::Direct2D::{D2D1CreateFactory, D2D1_FACTORY_TYPE_SINGLE_THREADED};

/// The strip's own proportions: the radius is a full semicircle, so every end runs the full height.
const H: f64 = vase_core::chrome::BAR_HEIGHT;
const R: f64 = H / 2.0;
const CY: f64 = H / 2.0;
const X0: f64 = 100.0;
const X1: f64 = 200.0;

fn factory() -> ID2D1Factory1 {
    unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None) }.unwrap()
}

/// A sweep in the wrong direction turns a notch into a bulge, which shows up here as a point on the
/// wrong side of the outline.
fn covers(geometry: &ID2D1PathGeometry1, x: f64, y: f64) -> bool {
    unsafe { geometry.FillContainsPoint(v(x, y), None, 0.05) }.unwrap().as_bool()
}

#[test]
fn a_tab_bulges_past_its_right_edge_and_is_notched_at_its_left() {
    let tab = tab(&factory(), X0, X1, false, R, H).unwrap();
    assert!(covers(&tab, (X0 + X1) / 2.0, CY), "the body");
    assert!(covers(&tab, X1 + R - 0.5, CY), "the bulge reaches a full radius past x1");
    assert!(!covers(&tab, X1 + R + 0.5, CY), "and no further");
    assert!(!covers(&tab, X0 + R - 0.5, CY), "the notch is carved a full radius into the body");
    assert!(covers(&tab, X0 + R + 0.5, CY), "and no deeper");
}

#[test]
fn a_capped_tab_bulges_left_where_a_notched_one_is_hollow() {
    let tab = tab(&factory(), X0, X1, true, R, H).unwrap();
    assert!(covers(&tab, X0 - R + 0.5, CY), "the cap bulges a full radius left of x0");
    assert!(!covers(&tab, X0 - R - 0.5, CY), "and no further");
    assert!(covers(&tab, X0 + 0.5, CY), "the body runs up to its own edge");
}

#[test]
fn the_lead_pill_caps_left_and_bulges_right() {
    let width = 36.0;
    let lead = lead(&factory(), width, R, H).unwrap();
    assert!(covers(&lead, width + R - 0.5, CY), "the bulge the first tab's notch nests into");
    assert!(covers(&lead, 0.5, CY), "the cap reaches the strip's left edge");
    assert!(!covers(&lead, 0.5, 0.5), "but rounds the corner away");
}
