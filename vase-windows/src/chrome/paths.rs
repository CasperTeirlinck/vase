//! The powerline outlines, as Direct2D geometry.
//!
//! Direct2D's y runs down where the core's bar-local coordinates run up, but every shape here is
//! symmetric about the strip's centre line, so only the sweep directions differ.

use windows::core::Result;
use windows::Win32::Graphics::Direct2D::Common::{D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_END_CLOSED, D2D_SIZE_F};
use windows::Win32::Graphics::Direct2D::{
    ID2D1Factory1, ID2D1GeometrySink, ID2D1PathGeometry1, D2D1_ARC_SEGMENT, D2D1_ARC_SIZE_SMALL, D2D1_SWEEP_DIRECTION_CLOCKWISE, D2D1_SWEEP_DIRECTION_COUNTER_CLOCKWISE,
};
use windows_numerics::Vector2;

fn v(x: f64, y: f64) -> Vector2 {
    Vector2 { X: x as f32, Y: y as f32 }
}

/// A quarter arc to `(x, y)`, curving `clockwise` on screen.
fn arc(sink: &ID2D1GeometrySink, x: f64, y: f64, r: f64, clockwise: bool) {
    unsafe {
        sink.AddArc(&D2D1_ARC_SEGMENT {
            point: v(x, y),
            size: D2D_SIZE_F { width: r as f32, height: r as f32 },
            rotationAngle: 0.0,
            sweepDirection: if clockwise { D2D1_SWEEP_DIRECTION_CLOCKWISE } else { D2D1_SWEEP_DIRECTION_COUNTER_CLOCKWISE },
            arcSize: D2D1_ARC_SIZE_SMALL,
        })
    }
}

/// A tab: a convex-right bulge at `x1`, and at `x0` either a concave notch the previous tab's bulge
/// nests into, or a rounded cap when the tab starts the bar.
pub fn tab(factory: &ID2D1Factory1, x0: f64, x1: f64, cap_left: bool, r: f64, h: f64) -> Result<ID2D1PathGeometry1> {
    let cy = h / 2.0;
    let path = unsafe { factory.CreatePathGeometry()? };
    let sink = unsafe { path.Open()? };
    unsafe {
        sink.BeginFigure(v(x0, 0.0), D2D1_FIGURE_BEGIN_FILLED);
        sink.AddLine(v(x1, 0.0));
        sink.AddLine(v(x1, cy - r));
        // Right bulge: 12 o'clock → 3 → 6 around (x1, cy).
        arc(&sink, x1 + r, cy, r, true);
        arc(&sink, x1, cy + r, r, true);
        sink.AddLine(v(x1, h));
        sink.AddLine(v(x0, h));
        sink.AddLine(v(x0, cy + r));
        if cap_left {
            // Rounded cap bulging left, matching the strip's corner: 6 o'clock → 9 → 12.
            arc(&sink, x0 - r, cy, r, true);
            arc(&sink, x0, cy - r, r, true);
        } else {
            // Notch carved into the tab: 6 o'clock → 3 → 12, curving back on itself.
            arc(&sink, x0 + r, cy, r, false);
            arc(&sink, x0, cy - r, r, false);
        }
        sink.AddLine(v(x0, 0.0));
        sink.EndFigure(D2D1_FIGURE_END_CLOSED);
        sink.Close()?;
    }
    Ok(path)
}

/// The leading mark block: a rounded-left cap matching the strip, and a convex-right bulge at
/// `width` that the first tab's notch nests into.
pub fn lead(factory: &ID2D1Factory1, width: f64, r: f64, h: f64) -> Result<ID2D1PathGeometry1> {
    let cy = h / 2.0;
    let cap = h / 2.0;
    let path = unsafe { factory.CreatePathGeometry()? };
    let sink = unsafe { path.Open()? };
    unsafe {
        sink.BeginFigure(v(cap, 0.0), D2D1_FIGURE_BEGIN_FILLED);
        sink.AddLine(v(width, 0.0));
        sink.AddLine(v(width, cy - r));
        arc(&sink, width + r, cy, r, true);
        arc(&sink, width, cy + r, r, true);
        sink.AddLine(v(width, h));
        sink.AddLine(v(cap, h));
        arc(&sink, 0.0, cy, cap, true);
        arc(&sink, cap, 0.0, cap, true);
        sink.EndFigure(D2D1_FIGURE_END_CLOSED);
        sink.Close()?;
    }
    Ok(path)
}

/// The brand mark's silhouette, filling `area` in surface coordinates.
pub fn vase_mark(factory: &ID2D1Factory1, area: vase_core::geometry::Rect, h: f64) -> Result<ID2D1PathGeometry1> {
    let path = unsafe { factory.CreatePathGeometry()? };
    let sink = unsafe { path.Open()? };
    // The core's polygon stands the vase up in a bottom-left origin, so flip it back for Direct2D.
    let points: Vec<(f64, f64)> = vase_core::chrome::theme::vase_mark().polygon(area).into_iter().map(|(x, y)| (x, h - y)).collect();
    unsafe {
        let Some(first) = points.first() else { return Ok(path) };
        sink.BeginFigure(v(first.0, first.1), D2D1_FIGURE_BEGIN_FILLED);
        for (x, y) in &points[1..] {
            sink.AddLine(v(*x, *y));
        }
        sink.EndFigure(D2D1_FIGURE_END_CLOSED);
        sink.Close()?;
    }
    Ok(path)
}

#[cfg(test)]
#[path = "paths_test.rs"]
mod tests;
