//! App icons, resolved once per app and kept as Direct2D bitmaps.
//!
//! Resolving one means asking the shell, which costs tens of milliseconds. That has to happen off
//! the daemon's thread: the low-level input hooks are delivered there, so every millisecond spent
//! blocking is a millisecond of stuttering cursor. A worker turns names into pixels and the daemon's
//! thread only uploads them.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};

use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, LPARAM, SIZE, WPARAM};
use windows::Win32::Graphics::Direct2D::Common::{D2D1_ALPHA_MODE_PREMULTIPLIED, D2D_SIZE_U};
use windows::Win32::Graphics::Direct2D::{ID2D1Bitmap1, D2D1_BITMAP_PROPERTIES1};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT, DXGI_FORMAT_B8G8R8A8_UNORM};
use windows::Win32::Graphics::Gdi::{DeleteObject, HPALETTE};
use windows::Win32::Graphics::Imaging::{CLSID_WICImagingFactory, IWICImagingFactory, WICBitmapUsePremultipliedAlpha};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED};
use windows::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Shell::{IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_ICONONLY};
use windows::Win32::UI::WindowsAndMessaging::PostThreadMessageW;

use super::gpu::Gpu;
use crate::win32::{from_wide, to_wide};

/// Asked of the shell at 32 px: the largest the bar or a list row ever draws, scaled down.
const ICON_PX: i32 = 32;

/// A resolved icon as plain pixels, which is all that can cross a thread boundary: a Direct2D
/// bitmap belongs to the device that made it.
struct Image {
    size: D2D_SIZE_U,
    /// Premultiplied BGRA, row-major.
    pixels: Vec<u8>,
}

pub struct Icons {
    /// Misses are cached too, so a name with no icon is looked up once.
    cache: HashMap<String, Option<ID2D1Bitmap1>>,
    /// Names the worker is still resolving, so each is asked for once.
    pending: HashSet<String>,
    requests: Sender<String>,
    resolved: Receiver<(String, Option<Image>)>,
}

impl Default for Icons {
    fn default() -> Self {
        let (requests, resolved) = spawn_resolver();
        Icons { cache: HashMap::new(), pending: HashSet::new(), requests, resolved }
    }
}

impl Icons {
    pub fn get(&self, app: &str) -> Option<&ID2D1Bitmap1> {
        self.cache.get(app)?.as_ref()
    }

    /// Ask for `app`'s icon. Returns immediately; the icon appears once `collect` picks it up.
    pub fn warm(&mut self, app: &str) {
        if self.cache.contains_key(app) || !self.pending.insert(app.to_string()) {
            return;
        }
        let _ = self.requests.send(app.to_string());
    }

    /// Upload whatever the worker has finished. Cheap when there is nothing waiting, so every redraw
    /// can call it.
    pub fn collect(&mut self, gpu: &Gpu) {
        while let Ok((app, image)) = self.resolved.try_recv() {
            self.pending.remove(&app);
            self.cache.insert(app, image.and_then(|image| upload(gpu, &image)));
        }
    }
}

/// The worker: one shell lookup at a time, in the order asked. Each result wakes the daemon's run
/// loop, which is otherwise idle and would leave the icon uncollected until something else redrew.
fn spawn_resolver() -> (Sender<String>, Receiver<(String, Option<Image>)>) {
    let (requests, inbox) = channel::<String>();
    let (outbox, resolved) = channel();
    let daemon_thread = unsafe { GetCurrentThreadId() };
    std::thread::spawn(move || {
        // The shell calls below are COM, and this is not the thread the daemon initialized.
        let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        // Enumerated once: the Applications folder holds hundreds of entries, and walking it per
        // lookup was most of what made resolving an icon slow.
        let installed: HashMap<String, String> = crate::backend::installed_apps().into_iter().collect();
        for app in inbox {
            let image = icon_source(&app, &installed).and_then(|target| load(&target));
            if outbox.send((app, image)).is_err() {
                return;
            }
            let _ = unsafe { PostThreadMessageW(daemon_thread, crate::ICONS_RESOLVED, WPARAM(0), LPARAM(0)) };
        }
    });
    (requests, resolved)
}

/// What the shell should be asked to draw for `app`.
///
/// Windows has no name-to-icon service the way `NSWorkspace` does, so vase resolves the name back to
/// something the shell knows. The two names it hands out come from different places: a window's app
/// is its executable's stem, and a picker launch row's is an installed app's display name.
fn icon_source(app: &str, installed: &HashMap<String, String>) -> Option<String> {
    match running_exe(app) {
        Some(exe) => Some(exe.to_string_lossy().into_owned()),
        None => installed.get(app).cloned(),
    }
}

/// The executable of a running process named `app`.
fn running_exe(app: &str) -> Option<PathBuf> {
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }.ok()?;
    let mut entry = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
    let mut found = None;
    if unsafe { Process32FirstW(snapshot, &mut entry) }.is_ok() {
        loop {
            let nul = entry.szExeFile.iter().position(|c| *c == 0).unwrap_or(entry.szExeFile.len());
            if Path::new(&from_wide(&entry.szExeFile, nul)).file_stem().and_then(|s| s.to_str()) == Some(app) {
                found = crate::backend::exe_path(entry.th32ProcessID);
                break;
            }
            if unsafe { Process32NextW(snapshot, &mut entry) }.is_err() {
                break;
            }
        }
    }
    let _ = unsafe { CloseHandle(snapshot) };
    found
}

/// The pixels the shell draws for `target`, which is an executable's path or an app's shell path.
///
/// Through the shell item rather than the file: a packaged app is not a file at all, and its logo
/// lives in its manifest where only the shell can reach it.
fn load(target: &str) -> Option<Image> {
    let wide = to_wide(target);
    unsafe {
        let item: IShellItemImageFactory = SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None).ok()?;
        let bitmap = item.GetImage(SIZE { cx: ICON_PX, cy: ICON_PX }, SIIGBF_ICONONLY).ok()?;
        // WIC is the route from a GDI bitmap to the premultiplied pixels Direct2D wants.
        let wic: IWICImagingFactory = CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER).ok()?;
        let source = wic.CreateBitmapFromHBITMAP(bitmap, HPALETTE::default(), WICBitmapUsePremultipliedAlpha);
        let _ = DeleteObject(bitmap.into());
        let source = source.ok()?;
        let (mut width, mut height) = (0, 0);
        source.GetSize(&mut width, &mut height).ok()?;
        let stride = width * 4;
        let mut pixels = vec![0u8; (stride * height) as usize];
        source.CopyPixels(std::ptr::null(), stride, &mut pixels).ok()?;
        Some(Image { size: D2D_SIZE_U { width, height }, pixels })
    }
}

fn upload(gpu: &Gpu, image: &Image) -> Option<ID2D1Bitmap1> {
    let props = D2D1_BITMAP_PROPERTIES1 {
        pixelFormat: windows::Win32::Graphics::Direct2D::Common::D2D1_PIXEL_FORMAT { format: DXGI_FORMAT(DXGI_FORMAT_B8G8R8A8_UNORM.0), alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED },
        dpiX: 96.0,
        dpiY: 96.0,
        ..Default::default()
    };
    unsafe { gpu.d2d.CreateBitmap(image.size, Some(image.pixels.as_ptr() as *const _), image.size.width * 4, &props) }.ok()
}

#[cfg(test)]
#[path = "icons_test.rs"]
mod tests;
