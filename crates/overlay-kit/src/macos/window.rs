use objc2_core_foundation::{
    CFBoolean, CFDictionary, CFNumber, CFRetained, CFString, CFType, CGPoint, CGRect, CGSize,
};
use objc2_core_graphics::{
    CGWindowListCopyWindowInfo, CGWindowListOption, kCGNullWindowID, kCGWindowBounds,
    kCGWindowIsOnscreen, kCGWindowLayer, kCGWindowNumber, kCGWindowOwnerPID,
};

use crate::geometry::CheckedRect;

/// Snapshot of an on-screen window from the Core Graphics window list.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CgWindowInfo {
    pub window_id: u32,
    pub owner_pid: i32,
    pub layer: i32,
    pub is_onscreen: Option<bool>,
    /// Window bounds in Core Graphics coordinates (top-left origin).
    pub cg_bounds: CGRect,
}

/// Enumerate on-screen, non-desktop windows.
pub fn onscreen_windows() -> Vec<CgWindowInfo> {
    let options =
        CGWindowListOption::OptionOnScreenOnly | CGWindowListOption::ExcludeDesktopElements;
    let Some(window_infos) = CGWindowListCopyWindowInfo(options, kCGNullWindowID) else {
        return Vec::new();
    };

    let typed = unsafe { window_infos.cast_unchecked::<CFDictionary<CFString, CFType>>() };
    typed
        .iter()
        .filter_map(|dict| parse_window(&dict))
        .collect()
}

fn parse_window(dict: &CFDictionary<CFString, CFType>) -> Option<CgWindowInfo> {
    let window_id = cf_dict_u32(dict, unsafe { kCGWindowNumber })?;
    let owner_pid = cf_dict_i32(dict, unsafe { kCGWindowOwnerPID })?;
    let layer = cf_dict_i32(dict, unsafe { kCGWindowLayer })?;
    let is_onscreen = cf_dict_bool(dict, unsafe { kCGWindowIsOnscreen });

    let bounds_value = dict.get(unsafe { kCGWindowBounds })?;
    let bounds_opaque: CFRetained<CFDictionary> = bounds_value.downcast::<CFDictionary>().ok()?;
    let bounds_dict = unsafe { bounds_opaque.cast_unchecked::<CFString, CFType>() };
    let cg_bounds = cg_rect_from_bounds(bounds_dict)?;

    Some(CgWindowInfo {
        window_id,
        owner_pid,
        layer,
        is_onscreen,
        cg_bounds,
    })
}

fn cg_rect_from_bounds(bounds: &CFDictionary<CFString, CFType>) -> Option<CGRect> {
    let x = cf_dict_f64(bounds, "X")?;
    let y = cf_dict_f64(bounds, "Y")?;
    let width = cf_dict_f64(bounds, "Width")?;
    let height = cf_dict_f64(bounds, "Height")?;
    let rect = CGRect::new(CGPoint::new(x, y), CGSize::new(width, height));
    CheckedRect::try_from(rect).ok().map(|_| rect)
}

fn cf_dict_i32(dict: &CFDictionary<CFString, CFType>, key: &CFString) -> Option<i32> {
    dict.get(key)?.downcast_ref::<CFNumber>()?.as_i32()
}

fn cf_dict_u32(dict: &CFDictionary<CFString, CFType>, key: &CFString) -> Option<u32> {
    u32::try_from(cf_dict_i32(dict, key)?).ok()
}

fn cf_dict_bool(dict: &CFDictionary<CFString, CFType>, key: &CFString) -> Option<bool> {
    Some(dict.get(key)?.downcast_ref::<CFBoolean>()?.value())
}

fn cf_dict_f64(dict: &CFDictionary<CFString, CFType>, key: &str) -> Option<f64> {
    let key = CFString::from_str(key);
    dict.get(&key)?.downcast_ref::<CFNumber>()?.as_f64()
}
