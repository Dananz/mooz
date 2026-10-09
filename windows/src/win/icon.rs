//! Builds HICONs from the raw RGBA assets with CreateIconIndirect, so no
//! image decoder or resource compiler is needed for the tray glyphs.

use windows_sys::Win32::Graphics::Gdi::{
    CreateBitmap, CreateDIBSection, DeleteObject, GetDC, ReleaseDC, BITMAPINFO, BITMAPV5HEADER,
    BI_BITFIELDS, DIB_RGB_COLORS,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateIconIndirect, GetSystemMetrics, HICON, ICONINFO, SM_CXSMICON,
};

const TRAY_LIGHT: &[u8] = include_bytes!("../../assets/tray-light.rgba");
const TRAY_DARK: &[u8] = include_bytes!("../../assets/tray-dark.rgba");
/// Sizes packed into the tray assets, smallest first.
const TRAY_SIZES: [i32; 6] = [16, 20, 24, 32, 40, 48];

/// The tray glyph for the current theme at the system small-icon size.
/// `dimmed` fades it for the paused state.
pub fn tray_icon(light_taskbar: bool, dimmed: bool) -> HICON {
    let data = if light_taskbar { TRAY_LIGHT } else { TRAY_DARK };
    let wanted = unsafe { GetSystemMetrics(SM_CXSMICON) };
    let size = TRAY_SIZES.iter().copied().find(|s| *s >= wanted).unwrap_or(48);
    let offset: usize = TRAY_SIZES
        .iter()
        .take_while(|s| **s != size)
        .map(|s| (*s * *s * 4) as usize)
        .sum();
    let len = (size * size * 4) as usize;
    from_rgba(&data[offset..offset + len], size, if dimmed { 0.4 } else { 1.0 })
}

/// Straight-alpha RGBA, top row first, into a 32-bit icon.
fn from_rgba(rgba: &[u8], size: i32, alpha: f32) -> HICON {
    unsafe {
        let mut header: BITMAPV5HEADER = std::mem::zeroed();
        header.bV5Size = std::mem::size_of::<BITMAPV5HEADER>() as u32;
        header.bV5Width = size;
        header.bV5Height = -size; // top-down
        header.bV5Planes = 1;
        header.bV5BitCount = 32;
        header.bV5Compression = BI_BITFIELDS;
        header.bV5RedMask = 0x00FF_0000;
        header.bV5GreenMask = 0x0000_FF00;
        header.bV5BlueMask = 0x0000_00FF;
        header.bV5AlphaMask = 0xFF00_0000;

        let dc = GetDC(std::ptr::null_mut());
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let color = CreateDIBSection(
            dc,
            &header as *const BITMAPV5HEADER as *const BITMAPINFO,
            DIB_RGB_COLORS,
            &mut bits,
            std::ptr::null_mut(),
            0,
        );
        ReleaseDC(std::ptr::null_mut(), dc);
        if color.is_null() || bits.is_null() {
            return std::ptr::null_mut();
        }
        let out = std::slice::from_raw_parts_mut(bits as *mut u8, rgba.len());
        for (dst, src) in out.as_chunks_mut::<4>().0.iter_mut().zip(rgba.as_chunks::<4>().0) {
            dst[0] = src[2];
            dst[1] = src[1];
            dst[2] = src[0];
            dst[3] = (f32::from(src[3]) * alpha).round() as u8;
        }

        // Monochrome mask, ignored for 32-bit icons with alpha but required.
        // Rows are WORD-aligned.
        let mask_bits = vec![0u8; (((size + 15) / 16) * 2 * size) as usize];
        let mask = CreateBitmap(size, size, 1, 1, mask_bits.as_ptr().cast());

        let info = ICONINFO { fIcon: 1, xHotspot: 0, yHotspot: 0, hbmMask: mask, hbmColor: color };
        let icon = CreateIconIndirect(&info);
        DeleteObject(color);
        DeleteObject(mask);
        icon
    }
}
