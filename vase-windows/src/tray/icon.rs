//! The tray icon's bitmap, rasterized once from the same mark the tab bar draws.

use windows::Win32::Graphics::Direct2D::Common::{D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT};
use windows::Win32::Graphics::Direct2D::{D2D1CreateFactory, ID2D1Factory1, D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_TYPE_DEFAULT};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::{CreateBitmap, CreateDIBSection, DeleteObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS};
use windows::Win32::Graphics::Imaging::{CLSID_WICImagingFactory, GUID_WICPixelFormat32bppPBGRA, IWICImagingFactory, WICBitmapCacheOnLoad, WICBitmapLockRead, WICRect};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
use windows::Win32::UI::WindowsAndMessaging::{CreateIconIndirect, HICON, ICONINFO};

use vase_core::chrome::theme::{palette, vase_mark, Role};
use vase_core::geometry::Rect;

/// The largest size the shell scales down from for a single-image icon.
const SIZE: u32 = 32;
/// Breathing room, so the mark does not touch the icon's edge.
const MARGIN: f64 = 1.0;

/// The mark in the theme's accent, as an icon the notification area can show.
pub fn vase_mark_icon() -> Option<HICON> {
    let pixels = render()?;
    unsafe {
        let mut bits = std::ptr::null_mut();
        // Negative height: top-down, matching the order WIC hands the rows back.
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: SIZE as i32,
                biHeight: -(SIZE as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let color = CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0).ok()?;
        std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits as *mut u8, pixels.len());
        // A 32-bit color bitmap carries its own alpha, so the mask only has to exist.
        let mask = CreateBitmap(SIZE as i32, SIZE as i32, 1, 1, None);
        let icon = CreateIconIndirect(&ICONINFO { fIcon: true.into(), hbmMask: mask, hbmColor: color, ..Default::default() }).ok();
        let _ = DeleteObject(color.into());
        let _ = DeleteObject(mask.into());
        icon
    }
}

/// The mark drawn into a premultiplied BGRA buffer, row-major and top-down.
fn render() -> Option<Vec<u8>> {
    unsafe {
        let wic: IWICImagingFactory = CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER).ok()?;
        let bitmap = wic.CreateBitmap(SIZE, SIZE, &GUID_WICPixelFormat32bppPBGRA, WICBitmapCacheOnLoad).ok()?;
        let factory: ID2D1Factory1 = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None).ok()?;
        let props = D2D1_RENDER_TARGET_PROPERTIES {
            r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
            pixelFormat: D2D1_PIXEL_FORMAT { format: DXGI_FORMAT_B8G8R8A8_UNORM, alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED },
            dpiX: 96.0,
            dpiY: 96.0,
            ..Default::default()
        };
        let target = factory.CreateWicBitmapRenderTarget(&bitmap, &props).ok()?;

        let h = SIZE as f64 - 2.0 * MARGIN;
        let w = h * vase_mark().aspect;
        let area = Rect::new((SIZE as f64 - w) / 2.0, MARGIN, w, h);
        let geometry = crate::chrome::paths::vase_mark(&factory, area, SIZE as f64).ok()?;
        let accent = palette().color(Role::Accent);
        let brush = target.CreateSolidColorBrush(&D2D1_COLOR_F { r: accent[0] as f32, g: accent[1] as f32, b: accent[2] as f32, a: accent[3] as f32 }, None).ok()?;
        target.BeginDraw();
        target.Clear(Some(&D2D1_COLOR_F::default()));
        target.FillGeometry(&geometry, &brush, None);
        target.EndDraw(None, None).ok()?;

        let rect = WICRect { X: 0, Y: 0, Width: SIZE as i32, Height: SIZE as i32 };
        let lock = bitmap.Lock(&rect, WICBitmapLockRead.0 as u32).ok()?;
        let stride = lock.GetStride().ok()? as usize;
        let mut size = 0;
        let mut data = std::ptr::null_mut();
        lock.GetDataPointer(&mut size, &mut data).ok()?;
        let row = SIZE as usize * 4;
        let mut out = vec![0u8; row * SIZE as usize];
        for y in 0..SIZE as usize {
            std::ptr::copy_nonoverlapping(data.add(y * stride), out[y * row..].as_mut_ptr(), row);
        }
        Some(out)
    }
}

#[cfg(test)]
#[path = "icon_test.rs"]
mod tests;
