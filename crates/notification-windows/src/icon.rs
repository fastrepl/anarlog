#![cfg_attr(not(target_os = "windows"), allow(dead_code))]

use anlg_notification_interface::Notification;

pub(crate) fn expand_home(path: &str) -> String {
    let rest = path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\"));
    if let Some(rest) = rest
        && let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))
    {
        return std::path::Path::new(&home)
            .join(rest)
            .to_string_lossy()
            .into_owned();
    }
    path.to_string()
}

pub(crate) fn header_title(notification: &Notification, expanded: bool) -> &str {
    if expanded {
        notification.expanded_title()
    } else {
        notification.compact_title()
    }
}

fn parse_default_icon(spec: &str) -> Option<(String, i32)> {
    let spec = spec.trim().trim_matches('"');
    if spec.is_empty() {
        return None;
    }
    match spec.rsplit_once(',') {
        Some((path, index)) => Some((
            path.trim().trim_matches('"').to_string(),
            index.trim().parse().ok()?,
        )),
        None => Some((spec.to_string(), 0)),
    }
}

#[cfg(target_os = "windows")]
mod win {
    use super::expand_home;
    use anlg_notification_interface::{NotificationIcon, NotificationIconAsset};
    use windows::Win32::Foundation::GENERIC_READ;
    use windows::Win32::Graphics::Gdi::{
        BI_BITFIELDS, BITMAPINFO, BITMAPV5HEADER, CreateCompatibleDC, CreateDIBSection,
        DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, ReleaseDC, SelectObject,
    };
    use windows::Win32::Graphics::Imaging::{
        CLSID_WICImagingFactory, GUID_WICPixelFormat32bppBGRA, IWICImagingFactory,
        WICBitmapDitherTypeNone, WICBitmapPaletteTypeCustom, WICDecodeMetadataCacheOnDemand,
    };
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    };
    use windows::Win32::UI::Shell::{
        ASSOCF, ASSOCSTR_DEFAULTICON, AssocQueryStringW, ExtractIconExW, SHDefExtractIconW,
        SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON, SHGSI_ICON, SHGSI_LARGEICON, SHGetFileInfoW,
        SHGetStockIconInfo, SHSTOCKICONINFO, SIID_APPLICATION, SIID_INFO, SIID_VIDEOFILES,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateIconIndirect, DI_NORMAL, DestroyIcon, DrawIconEx, HICON, ICONINFO,
    };
    use windows::core::{PCWSTR, PWSTR};

    const ICON_PX: i32 = 32;

    pub(crate) fn load_notification_icon(icon: Option<&NotificationIcon>) -> Option<HICON> {
        match icon {
            None => load_app_icon(),
            Some(NotificationIcon::Hidden) => None,
            Some(NotificationIcon::Path { path }) => {
                load_icon_from_path(path).or_else(load_app_icon)
            }
            Some(NotificationIcon::BundleId { bundle_id }) => {
                load_icon_from_bundle_id(bundle_id).or_else(load_app_icon)
            }
            Some(NotificationIcon::SystemSymbol { name }) => {
                load_system_symbol(name).or_else(load_app_icon)
            }
            Some(NotificationIcon::Overlay { base, badge }) => load_overlay(base, badge),
        }
    }

    fn load_icon_asset(asset: &NotificationIconAsset) -> Option<HICON> {
        match asset {
            NotificationIconAsset::AppIcon => load_app_icon(),
            NotificationIconAsset::Calendar => load_calendar_icon(),
            NotificationIconAsset::SystemSymbol { name } => load_system_symbol(name),
            NotificationIconAsset::BundleId { bundle_id } => load_icon_from_bundle_id(bundle_id),
            NotificationIconAsset::Path { path } => load_icon_from_path(path),
        }
    }

    fn load_overlay(base: &NotificationIconAsset, badge: &NotificationIconAsset) -> Option<HICON> {
        let base_icon = load_icon_asset(base).or_else(load_app_icon)?;
        let Some(badge_icon) = load_icon_asset(badge) else {
            return Some(base_icon);
        };
        match compose_overlay(base_icon, badge_icon) {
            Some(composed) => {
                destroy_icon(Some(base_icon));
                destroy_icon(Some(badge_icon));
                Some(composed)
            }
            None => {
                destroy_icon(Some(badge_icon));
                Some(base_icon)
            }
        }
    }

    fn load_icon_from_bundle_id(bundle_id: &str) -> Option<HICON> {
        if looks_like_path(bundle_id) {
            return load_icon_from_path(bundle_id);
        }
        extract_named_executable(bundle_id)
    }

    fn load_icon_from_path(path: &str) -> Option<HICON> {
        let expanded = expand_home(path);
        extract_icon_at(&expanded, 0)
            .or_else(|| load_image_file(&expanded))
            .or_else(|| file_association_icon(&expanded))
    }

    fn load_app_icon() -> Option<HICON> {
        let exe = std::env::current_exe().ok()?;
        extract_icon_at(&exe.to_string_lossy(), 0).or_else(|| stock_icon(SIID_APPLICATION))
    }

    fn load_calendar_icon() -> Option<HICON> {
        assoc_default_icon(".ics").or_else(|| stock_icon(SIID_INFO))
    }

    fn load_system_symbol(name: &str) -> Option<HICON> {
        match name {
            "phone.fill" | "phone" => assoc_default_icon("tel").or_else(|| stock_icon(SIID_INFO)),
            "video.fill" | "video" => stock_icon(SIID_VIDEOFILES),
            "calendar" => load_calendar_icon(),
            _ => stock_icon(SIID_INFO),
        }
    }

    fn extract_named_executable(name: &str) -> Option<HICON> {
        let file_name = std::path::Path::new(name)
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or(name);
        if !file_name.rsplit('.').next().is_some_and(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "exe" | "dll" | "ico" | "cpl"
            )
        }) {
            return None;
        }
        extract_icon_at(file_name, 0)
    }

    fn extract_icon_at(path: &str, index: i32) -> Option<HICON> {
        let wide_path = wide(path);
        let mut large = HICON::default();
        unsafe {
            if SHDefExtractIconW(
                PCWSTR(wide_path.as_ptr()),
                index,
                0,
                Some(&mut large),
                None,
                ICON_PX as u32,
            )
            .is_ok()
                && !large.is_invalid()
            {
                return Some(large);
            }
            if ExtractIconExW(PCWSTR(wide_path.as_ptr()), index, Some(&mut large), None, 1) > 0
                && !large.is_invalid()
            {
                return Some(large);
            }
        }
        None
    }

    fn file_association_icon(path: &str) -> Option<HICON> {
        let wide_path = wide(path);
        let mut info = SHFILEINFOW::default();
        let result = unsafe {
            SHGetFileInfoW(
                PCWSTR(wide_path.as_ptr()),
                Default::default(),
                Some(&mut info),
                std::mem::size_of::<SHFILEINFOW>() as u32,
                SHGFI_ICON | SHGFI_LARGEICON,
            )
        };
        if result != 0 && !info.hIcon.is_invalid() {
            Some(info.hIcon)
        } else {
            None
        }
    }

    fn stock_icon(id: windows::Win32::UI::Shell::SHSTOCKICONID) -> Option<HICON> {
        let mut info = SHSTOCKICONINFO {
            cbSize: std::mem::size_of::<SHSTOCKICONINFO>() as u32,
            ..Default::default()
        };
        unsafe {
            SHGetStockIconInfo(id, SHGSI_ICON | SHGSI_LARGEICON, &mut info).ok()?;
        }
        if info.hIcon.is_invalid() {
            None
        } else {
            Some(info.hIcon)
        }
    }

    fn assoc_default_icon(assoc: &str) -> Option<HICON> {
        let wide_assoc = wide(assoc);
        let mut len = 0u32;
        unsafe {
            let _ = AssocQueryStringW(
                ASSOCF(0),
                ASSOCSTR_DEFAULTICON,
                PCWSTR(wide_assoc.as_ptr()),
                PCWSTR::null(),
                None,
                &mut len,
            );
        }
        if len == 0 {
            return None;
        }
        let mut buf = vec![0u16; len as usize];
        let status = unsafe {
            AssocQueryStringW(
                ASSOCF(0),
                ASSOCSTR_DEFAULTICON,
                PCWSTR(wide_assoc.as_ptr()),
                PCWSTR::null(),
                Some(PWSTR(buf.as_mut_ptr())),
                &mut len,
            )
        };
        if status.is_err() {
            return None;
        }
        let spec = String::from_utf16_lossy(&buf)
            .trim_end_matches('\0')
            .trim()
            .to_string();
        let (path, index) = super::parse_default_icon(&spec)?;
        extract_icon_at(&path, index)
    }

    fn load_image_file(path: &str) -> Option<HICON> {
        ensure_com();
        let wide_path = wide(path);
        unsafe {
            let factory: IWICImagingFactory =
                CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER).ok()?;
            let decoder = factory
                .CreateDecoderFromFilename(
                    PCWSTR(wide_path.as_ptr()),
                    None,
                    GENERIC_READ,
                    WICDecodeMetadataCacheOnDemand,
                )
                .ok()?;
            let frame = decoder.GetFrame(0).ok()?;
            let converter = factory.CreateFormatConverter().ok()?;
            converter
                .Initialize(
                    &frame,
                    &GUID_WICPixelFormat32bppBGRA,
                    WICBitmapDitherTypeNone,
                    None,
                    0.0,
                    WICBitmapPaletteTypeCustom,
                )
                .ok()?;
            let mut width = 0u32;
            let mut height = 0u32;
            converter.GetSize(&mut width, &mut height).ok()?;
            if width == 0 || height == 0 {
                return None;
            }
            let stride = width.checked_mul(4)?;
            let mut pixels = vec![0u8; stride.checked_mul(height)? as usize];
            converter
                .CopyPixels(std::ptr::null(), stride, &mut pixels)
                .ok()?;
            hicon_from_bgra(width as i32, height as i32, &pixels)
        }
    }

    fn compose_overlay(base: HICON, badge: HICON) -> Option<HICON> {
        unsafe {
            let mut bits = std::ptr::null_mut();
            let header = bitmap_header(ICON_PX, ICON_PX);
            let hdc_screen = GetDC(None);
            let color = CreateDIBSection(
                Some(hdc_screen),
                std::ptr::from_ref(&header).cast::<BITMAPINFO>(),
                DIB_RGB_COLORS,
                &mut bits,
                None,
                0,
            )
            .ok();
            let Some(color) = color else {
                let _ = ReleaseDC(None, hdc_screen);
                return None;
            };
            if !bits.is_null() {
                std::ptr::write_bytes(bits as *mut u8, 0, (ICON_PX * ICON_PX * 4) as usize);
            }
            let hdc = CreateCompatibleDC(Some(hdc_screen));
            if hdc.is_invalid() {
                let _ = DeleteObject(color.into());
                ReleaseDC(None, hdc_screen);
                return None;
            }
            let previous = SelectObject(hdc, color.into());
            let _ = DrawIconEx(hdc, 0, 0, base, ICON_PX, ICON_PX, 0, None, DI_NORMAL);
            let badge_size = ((ICON_PX as f64) * 0.54).round() as i32;
            let _ = DrawIconEx(
                hdc,
                ICON_PX - badge_size,
                ICON_PX - badge_size,
                badge,
                badge_size,
                badge_size,
                0,
                None,
                DI_NORMAL,
            );
            let _ = SelectObject(hdc, previous);
            let _ = DeleteDC(hdc);
            let _ = ReleaseDC(None, hdc_screen);
            let icon = icon_from_color_bitmap(color, ICON_PX, ICON_PX);
            let _ = DeleteObject(color.into());
            icon
        }
    }

    fn hicon_from_bgra(width: i32, height: i32, bgra: &[u8]) -> Option<HICON> {
        unsafe {
            let mut bits = std::ptr::null_mut();
            let header = bitmap_header(width, height);
            let hdc = GetDC(None);
            let color = CreateDIBSection(
                Some(hdc),
                std::ptr::from_ref(&header).cast::<BITMAPINFO>(),
                DIB_RGB_COLORS,
                &mut bits,
                None,
                0,
            )
            .ok();
            let _ = ReleaseDC(None, hdc);
            let color = color?;
            if bits.is_null() {
                let _ = DeleteObject(color.into());
                return None;
            }
            let byte_len = (width * height * 4) as usize;
            std::ptr::copy_nonoverlapping(bgra.as_ptr(), bits as *mut u8, bgra.len().min(byte_len));
            let icon = icon_from_color_bitmap(color, width, height);
            let _ = DeleteObject(color.into());
            icon
        }
    }

    fn icon_from_color_bitmap(
        color: windows::Win32::Graphics::Gdi::HBITMAP,
        width: i32,
        height: i32,
    ) -> Option<HICON> {
        unsafe {
            let mask = windows::Win32::Graphics::Gdi::CreateBitmap(width, height, 1, 1, None);
            if mask.is_invalid() {
                return None;
            }
            let info = ICONINFO {
                fIcon: true.into(),
                xHotspot: 0,
                yHotspot: 0,
                hbmMask: mask,
                hbmColor: color,
            };
            let icon = CreateIconIndirect(&info).ok();
            let _ = DeleteObject(mask.into());
            icon
        }
    }

    fn bitmap_header(width: i32, height: i32) -> BITMAPV5HEADER {
        BITMAPV5HEADER {
            bV5Size: std::mem::size_of::<BITMAPV5HEADER>() as u32,
            bV5Width: width,
            bV5Height: -height,
            bV5Planes: 1,
            bV5BitCount: 32,
            bV5Compression: BI_BITFIELDS,
            bV5RedMask: 0x00FF_0000,
            bV5GreenMask: 0x0000_FF00,
            bV5BlueMask: 0x0000_00FF,
            bV5AlphaMask: 0xFF00_0000,
            ..Default::default()
        }
    }

    fn ensure_com() {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }
    }

    fn looks_like_path(value: &str) -> bool {
        value.contains('\\') || value.contains('/') || std::path::Path::new(value).exists()
    }

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub(crate) fn destroy_icon(icon: Option<HICON>) {
        if let Some(icon) = icon {
            unsafe {
                let _ = DestroyIcon(icon);
            }
        }
    }
}

#[cfg(target_os = "windows")]
pub(crate) use win::{destroy_icon, load_notification_icon};

#[cfg(test)]
mod tests {
    use super::{expand_home, header_title};
    use anlg_notification_interface::{EventDetails, Notification};

    #[test]
    fn expands_home_prefixed_icon_paths() {
        let previous_home = std::env::var("HOME").ok();
        let previous_profile = std::env::var("USERPROFILE").ok();
        unsafe {
            std::env::set_var("HOME", "/home/anarlog");
            std::env::remove_var("USERPROFILE");
        }
        assert_eq!(
            expand_home("~/icons/zoom.png"),
            "/home/anarlog/icons/zoom.png"
        );
        assert_eq!(expand_home("/usr/share/zoom.png"), "/usr/share/zoom.png");
        match previous_home {
            Some(home) => unsafe { std::env::set_var("HOME", home) },
            None => unsafe { std::env::remove_var("HOME") },
        }
        match previous_profile {
            Some(profile) => unsafe { std::env::set_var("USERPROFILE", profile) },
            None => {}
        }
    }

    #[test]
    fn parses_shell_default_icon_specs() {
        assert_eq!(
            super::parse_default_icon(r#""C:\Windows\System32\imageres.dll",-102"#),
            Some((r"C:\Windows\System32\imageres.dll".to_string(), -102))
        );
        assert_eq!(
            super::parse_default_icon("outlook.exe"),
            Some(("outlook.exe".to_string(), 0))
        );
        assert_eq!(super::parse_default_icon("   "), None);
    }

    #[test]
    fn expanded_headers_use_the_event_name() {
        let notification = Notification::builder()
            .title("Upcoming event")
            .message("")
            .event_details(EventDetails {
                what: "Design sync".to_string(),
                timezone: None,
                location: None,
            })
            .build();
        assert_eq!(header_title(&notification, false), "Upcoming event");
        assert_eq!(header_title(&notification, true), "Design sync");
    }
}
