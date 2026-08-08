use crate::backend::*;
use crate::geometry::Rect;
use crate::tree::WindowId;

fn win(layer: i64, title: &str, w: f64, h: f64) -> WindowInfo {
    WindowInfo { id: WindowId(1), pid: 1, app: "App".into(), title: title.into(), frame: Rect::new(0.0, 0.0, w, h), layer }
}

#[test]
fn normal_titled_window_is_manageable() {
    assert!(manageable(&win(0, "Editor", 800.0, 600.0)));
}

#[test]
fn overlay_layer_and_untitled_and_tiny_are_not_manageable() {
    assert!(!manageable(&win(25, "Menu", 800.0, 600.0)));
    assert!(!manageable(&win(0, "", 800.0, 600.0)));
    assert!(!manageable(&win(0, "Tip", 10.0, 10.0)));
}
