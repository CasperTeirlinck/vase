use super::*;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};

#[test]
fn the_mark_rasterizes_into_a_square_of_its_own() {
    // WIC is a COM object, and the daemon's own apartment is set up long before the tray exists.
    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    let pixels = render().expect("the mark has to rasterize");
    assert_eq!(pixels.len(), (SIZE * SIZE * 4) as usize);

    let alpha: Vec<u8> = pixels.chunks(4).map(|p| p[3]).collect();
    let opaque = alpha.iter().filter(|a| **a > 200).count();
    // A silhouette, not a blank square and not a filled one: it covers a fair share of the icon and
    // leaves the corners clear.
    assert!((200..800).contains(&opaque), "{opaque} opaque pixels of {}", alpha.len());
    for corner in [0, SIZE as usize - 1, alpha.len() - SIZE as usize, alpha.len() - 1] {
        assert_eq!(alpha[corner], 0, "corner {corner} should be transparent");
    }
}
