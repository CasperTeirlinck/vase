//! App icons, resolved once per app and kept as Direct2D bitmaps.

use std::collections::HashMap;

use windows::core::PCWSTR;
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;
use windows::Win32::Graphics::Imaging::{CLSID_WICImagingFactory, GUID_WICPixelFormat32bppPBGRA, IWICImagingFactory, WICBitmapDitherTypeNone, WICBitmapPaletteTypeMedianCut};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
use windows::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON};
use windows::Win32::UI::WindowsAndMessaging::DestroyIcon;

use super::gpu::Gpu;
use crate::win32::to_wide;

#[derive(Default)]
pub struct Icons {
    /// Misses are cached too, so a name with no icon is looked up once.
    cache: HashMap<String, Option<ID2D1Bitmap1>>,
}

impl Icons {
    pub fn get(&self, app: &str) -> Option<&ID2D1Bitmap1> {
        self.cache.get(app)?.as_ref()
    }

    pub fn warm(&mut self, gpu: &Gpu, app: &str) {
        if self.cache.contains_key(app) {
            return;
        }
        let bitmap = shortcut_for(app).and_then(|path| load(gpu, &path));
        self.cache.insert(app.to_string(), bitmap);
    }
}

/// The Start Menu shortcut whose name matches `app`.
///
/// Windows has no name-to-icon service the way `NSWorkspace` does, so the shortcut is the only
/// handle vase has on an app it knows only by name. A running app with no Start Menu entry gets no
/// icon until `WindowInfo` carries its executable path.
fn shortcut_for(app: &str) -> Option<std::path::PathBuf> {
    crate::paths::start_menu_dirs().iter().flat_map(|dir| crate::backend::shortcuts(dir)).find(|p| p.file_stem().and_then(|s| s.to_str()) == Some(app))
}

fn load(gpu: &Gpu, path: &std::path::Path) -> Option<ID2D1Bitmap1> {
    let wide = to_wide(&path.to_string_lossy());
    let mut info = SHFILEINFOW::default();
    let ok = unsafe { SHGetFileInfoW(PCWSTR(wide.as_ptr()), Default::default(), Some(&mut info), std::mem::size_of::<SHFILEINFOW>() as u32, SHGFI_ICON | SHGFI_LARGEICON) };
    if ok == 0 || info.hIcon.is_invalid() {
        return None;
    }
    let bitmap = unsafe {
        // WIC is the only route from an HICON to a premultiplied bitmap Direct2D can draw.
        let wic: IWICImagingFactory = CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER).ok()?;
        let source = wic.CreateBitmapFromHICON(info.hIcon).ok()?;
        let converter = wic.CreateFormatConverter().ok()?;
        converter.Initialize(&source, &GUID_WICPixelFormat32bppPBGRA, WICBitmapDitherTypeNone, None, 0.0, WICBitmapPaletteTypeMedianCut).ok()?;
        gpu.d2d.CreateBitmapFromWicBitmap(&converter, None).ok()
    };
    let _ = unsafe { DestroyIcon(info.hIcon) };
    bitmap
}
