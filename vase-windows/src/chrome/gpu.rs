//! The shared graphics devices, and the composition surfaces vase paints its chrome into.

use std::cell::RefCell;
use std::collections::HashMap;

use windows::core::{Interface, Result, HSTRING, PCWSTR};
use windows::Win32::Foundation::{HMODULE, HWND, POINT};
use windows::Win32::Graphics::Direct2D::Common::{D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT};
use windows::Win32::Graphics::Direct2D::{
    D2D1CreateFactory, ID2D1Bitmap1, ID2D1DeviceContext, ID2D1Factory1, D2D1_BITMAP_OPTIONS_CANNOT_DRAW, D2D1_BITMAP_OPTIONS_TARGET, D2D1_BITMAP_PROPERTIES1, D2D1_DEVICE_CONTEXT_OPTIONS_NONE,
    D2D1_FACTORY_TYPE_SINGLE_THREADED,
};
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::{D3D11CreateDevice, ID3D11Device, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION};
use windows::Win32::Graphics::DirectComposition::{DCompositionCreateDevice3, IDCompositionDesktopDevice, IDCompositionSurface, IDCompositionTarget, IDCompositionVisual2};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteInlineObject, IDWriteTextFormat, IDWriteTextLayout, DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT_NORMAL, DWRITE_TEXT_METRICS, DWRITE_TRIMMING, DWRITE_TRIMMING_GRANULARITY_CHARACTER, DWRITE_WORD_WRAPPING_NO_WRAP,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_ALPHA_MODE_PREMULTIPLIED, DXGI_FORMAT_B8G8R8A8_UNORM};
use windows::Win32::Graphics::Dxgi::{IDXGIDevice, IDXGISurface};
use windows::Win32::Graphics::Gdi::{CombineRgn, CreateRectRgn, DeleteObject, SetWindowRgn, RGN_DIFF};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, LoadCursorW, RegisterClassW, SetWindowPos, ShowWindow, HWND_TOPMOST, IDC_ARROW, SWP_NOACTIVATE, SW_HIDE, SW_SHOWNOACTIVATE, WNDCLASSW, WS_EX_NOACTIVATE,
    WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

use vase_core::chrome::theme::{palette, Role};
use vase_core::geometry::Rect;

/// The vase chrome's window class. Registered once; the first `Surface` does it.
const CLASS: PCWSTR = windows::core::w!("vase_chrome");

/// Devices every surface shares. One D3D device backs the D2D device, which backs the composition
/// device, so all three agree about which GPU is drawing.
pub struct Gpu {
    pub d2d: ID2D1DeviceContext,
    pub factory: ID2D1Factory1,
    pub dwrite: IDWriteFactory,
    dcomp: IDCompositionDesktopDevice,
    /// Text formats by point size. Ellipsizing measures a label many times per redraw, and creating
    /// a format per measurement dominates the cost.
    formats: RefCell<HashMap<u64, IDWriteTextFormat>>,
    /// Trimming signs, cached for the same reason and keyed the same way.
    ellipses: RefCell<HashMap<u64, IDWriteInlineObject>>,
}

impl Gpu {
    pub fn new() -> Result<Gpu> {
        unsafe {
            let mut device: Option<ID3D11Device> = None;
            D3D11CreateDevice(None, D3D_DRIVER_TYPE_HARDWARE, HMODULE::default(), D3D11_CREATE_DEVICE_BGRA_SUPPORT, None, D3D11_SDK_VERSION, Some(&mut device), None, None)?;
            let dxgi: IDXGIDevice = device.unwrap().cast()?;
            let factory: ID2D1Factory1 = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let d2d = factory.CreateDevice(&dxgi)?.CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)?;
            let dcomp: IDCompositionDesktopDevice = DCompositionCreateDevice3(&dxgi)?;
            let dwrite: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            Ok(Gpu { d2d, factory, dwrite, dcomp, formats: RefCell::new(HashMap::new()), ellipses: RefCell::new(HashMap::new()) })
        }
    }

    /// Segoe UI Variable Text at `size`, the Windows UI face, so the chrome reads as native.
    pub fn format(&self, size: f64) -> Result<IDWriteTextFormat> {
        let key = size.to_bits();
        if let Some(f) = self.formats.borrow().get(&key) {
            return Ok(f.clone());
        }
        let format = unsafe {
            self.dwrite.CreateTextFormat(
                &HSTRING::from("Segoe UI Variable Text"),
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                size as f32,
                &HSTRING::from("en-us"),
            )?
        };
        self.formats.borrow_mut().insert(key, format.clone());
        Ok(format)
    }

    pub fn layout(&self, text: &str, size: f64) -> Result<IDWriteTextLayout> {
        let wide: Vec<u16> = text.encode_utf16().collect();
        unsafe { self.dwrite.CreateTextLayout(&wide, &self.format(size)?, f32::MAX, f32::MAX) }
    }

    /// A layout capped at `max`, ellipsized where it would overrun. The bar ellipsizes in the core
    /// against `measure`, but a list row's width is the painter's own, so it trims here.
    pub fn trimmed(&self, text: &str, size: f64, max: f64) -> Result<IDWriteTextLayout> {
        let format = self.format(size)?;
        let wide: Vec<u16> = text.encode_utf16().collect();
        let layout = unsafe { self.dwrite.CreateTextLayout(&wide, &format, max.max(0.0) as f32, f32::MAX)? };
        unsafe {
            layout.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
            let trimming = DWRITE_TRIMMING { granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER, delimiter: 0, delimiterCount: 0 };
            layout.SetTrimming(&trimming, &self.ellipsis(size)?)?;
        }
        Ok(layout)
    }

    fn ellipsis(&self, size: f64) -> Result<IDWriteInlineObject> {
        let key = size.to_bits();
        if let Some(sign) = self.ellipses.borrow().get(&key) {
            return Ok(sign.clone());
        }
        let sign = unsafe { self.dwrite.CreateEllipsisTrimmingSign(&self.format(size)?)? };
        self.ellipses.borrow_mut().insert(key, sign.clone());
        Ok(sign)
    }

    /// Width of `text`, including trailing whitespace: the bar pads with spaces, and dropping them
    /// would misplace everything after.
    pub fn measure(&self, text: &str, size: f64) -> f64 {
        let Ok(layout) = self.layout(text, size) else { return 0.0 };
        let mut m = DWRITE_TEXT_METRICS::default();
        if unsafe { layout.GetMetrics(&mut m) }.is_err() {
            return 0.0;
        }
        m.widthIncludingTrailingWhitespace as f64
    }

    pub fn commit(&self) {
        let _ = unsafe { self.dcomp.Commit() };
    }
}

/// A color from the active palette, as Direct2D wants it.
pub fn color(role: Role) -> D2D1_COLOR_F {
    let c = palette().color(role);
    D2D1_COLOR_F { r: c[0] as f32, g: c[1] as f32, b: c[2] as f32, a: c[3] as f32 }
}

/// One always-on-top, click-through, never-activating overlay window, composited with per-pixel alpha.
pub struct Surface {
    hwnd: HWND,
    _target: IDCompositionTarget,
    visual: IDCompositionVisual2,
    /// Reallocated whenever the surface changes size.
    surface: Option<IDCompositionSurface>,
    size: (u32, u32),
    visible: bool,
}

impl Surface {
    pub fn new(gpu: &Gpu) -> Result<Surface> {
        unsafe {
            register_class();
            // NOREDIRECTIONBITMAP: the composition device owns the pixels, so Windows allocates no
            // redirection surface and the window can be genuinely transparent.
            // NOACTIVATE keeps focus in the user's app. TRANSPARENT asks for clicks to fall through
            // to it, but a surface that covers a whole pane has to be hollowed out to mean it: see
            // `keep_only_the_frame`.
            let hwnd = CreateWindowExW(
                WS_EX_NOREDIRECTIONBITMAP | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TRANSPARENT | WS_EX_TOPMOST,
                CLASS,
                PCWSTR::null(),
                WS_POPUP,
                0,
                0,
                1,
                1,
                None,
                None,
                None,
                None,
            )?;
            let target = gpu.dcomp.CreateTargetForHwnd(hwnd, true)?;
            let visual = gpu.dcomp.CreateVisual()?;
            target.SetRoot(&visual)?;
            Ok(Surface { hwnd, _target: target, visual, surface: None, size: (0, 0), visible: false })
        }
    }

    /// Like `draw`, for a surface whose paint is an outline: the middle stays clickable.
    pub fn draw_outline(&mut self, gpu: &Gpu, rect: Rect, band: f64, paint: impl FnOnce(&ID2D1DeviceContext)) -> Result<()> {
        self.draw(gpu, rect, paint)?;
        self.keep_only_the_frame(band as i32);
        Ok(())
    }

    /// Move the window to `rect` and draw into it. `paint` runs with the origin already at the
    /// surface's top-left, so it can work in bar-local coordinates.
    pub fn draw(&mut self, gpu: &Gpu, rect: Rect, paint: impl FnOnce(&ID2D1DeviceContext)) -> Result<()> {
        let size = (rect.w.max(1.0) as u32, rect.h.max(1.0) as u32);
        unsafe {
            SetWindowPos(self.hwnd, Some(HWND_TOPMOST), rect.x as i32, rect.y as i32, size.0 as i32, size.1 as i32, SWP_NOACTIVATE)?;
            if self.surface.is_none() || self.size != size {
                let surface = gpu.dcomp.CreateSurface(size.0, size.1, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_ALPHA_MODE_PREMULTIPLIED)?;
                self.visual.SetContent(&surface)?;
                self.surface = Some(surface);
                self.size = size;
            }
            let surface = self.surface.as_ref().unwrap();
            let mut offset = POINT::default();
            let dxgi: IDXGISurface = surface.BeginDraw(None, &mut offset)?;
            let props = D2D1_BITMAP_PROPERTIES1 {
                pixelFormat: D2D1_PIXEL_FORMAT { format: DXGI_FORMAT_B8G8R8A8_UNORM, alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED },
                dpiX: 96.0,
                dpiY: 96.0,
                bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
                ..Default::default()
            };
            let bitmap: ID2D1Bitmap1 = gpu.d2d.CreateBitmapFromDxgiSurface(&dxgi, Some(&props))?;
            gpu.d2d.SetTarget(&bitmap);
            gpu.d2d.BeginDraw();
            // The composition surface is atlased, so BeginDraw hands back an offset into a shared
            // texture; shifting by it lets everything below draw from (0, 0).
            gpu.d2d.SetTransform(&windows_numerics::Matrix3x2::translation(offset.x as f32, offset.y as f32));
            gpu.d2d.Clear(Some(&D2D1_COLOR_F::default()));
            paint(&gpu.d2d);
            gpu.d2d.EndDraw(None, None)?;
            gpu.d2d.SetTarget(None);
            surface.EndDraw()?;
            if !self.visible {
                let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
                self.visible = true;
            }
        }
        Ok(())
    }

    /// Cut the middle out of the window, leaving a frame `band` pixels wide.
    ///
    /// A surface that covers a whole pane would otherwise swallow every click in it: hit-testing
    /// hands a top-level window the mouse whatever its styles say, and `WS_EX_TRANSPARENT` and an
    /// `HTTRANSPARENT` hit test both fail to give it back. A region is not a hint: the middle stops
    /// being part of the window, so the click lands on the app behind it.
    /// <https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowrgn>
    fn keep_only_the_frame(&self, band: i32) {
        let (w, h) = (self.size.0 as i32, self.size.1 as i32);
        unsafe {
            let frame = CreateRectRgn(0, 0, w, h);
            let middle = CreateRectRgn(band, band, w - band, h - band);
            CombineRgn(Some(frame), Some(frame), Some(middle), RGN_DIFF);
            let _ = DeleteObject(middle.into());
            // Takes ownership of the region on success, so it must not be freed here.
            SetWindowRgn(self.hwnd, Some(frame), true);
        }
    }

    pub fn hide(&mut self) {
        if self.visible {
            let _ = unsafe { ShowWindow(self.hwnd, SW_HIDE) };
            self.visible = false;
        }
    }
}

fn register_class() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        // A class with no cursor makes its windows keep whatever the cursor already was, which after
        // process start is the busy one: hovering the bar would show an hourglass forever.
        // <https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-wndclassw>
        let class = WNDCLASSW { lpfnWndProc: Some(wndproc), lpszClassName: CLASS, hCursor: unsafe { LoadCursorW(None, IDC_ARROW) }.unwrap_or_default(), ..Default::default() };
        unsafe { RegisterClassW(&class) };
    });
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: windows::Win32::Foundation::WPARAM, lparam: windows::Win32::Foundation::LPARAM) -> windows::Win32::Foundation::LRESULT {
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}
