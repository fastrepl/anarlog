use gdk_pixbuf::{InterpType, Pixbuf};
use gtk::IconTheme;
use gtk::prelude::IconThemeExt;

use anlg_notification_interface::{NotificationIcon, NotificationIconAsset};

const ICON_SIZE: i32 = 28;

pub(crate) fn pixbuf_for_icon(icon: Option<&NotificationIcon>) -> Option<Pixbuf> {
    match icon {
        None => default_app_icon(),
        Some(NotificationIcon::Hidden) => None,
        Some(NotificationIcon::BundleId { bundle_id }) => {
            icon_theme_pixbuf(bundle_id).or_else(default_app_icon)
        }
        Some(NotificationIcon::SystemSymbol { name }) => {
            icon_theme_pixbuf(system_symbol_icon_name(name)).or_else(default_app_icon)
        }
        Some(NotificationIcon::Path { path }) => pixbuf_from_path(path).or_else(default_app_icon),
        Some(NotificationIcon::Overlay { base, badge }) => {
            let base_image = pixbuf_for_asset(base).or_else(default_app_icon)?;
            match pixbuf_for_asset(badge) {
                Some(badge_image) => Some(compose_overlay(&base_image, &badge_image)),
                None => Some(base_image),
            }
        }
    }
}

fn pixbuf_for_asset(asset: &NotificationIconAsset) -> Option<Pixbuf> {
    match asset {
        NotificationIconAsset::AppIcon => default_app_icon(),
        NotificationIconAsset::Calendar => icon_theme_pixbuf("x-office-calendar")
            .or_else(|| icon_theme_pixbuf("office-calendar"))
            .or_else(|| icon_theme_pixbuf("calendar")),
        NotificationIconAsset::SystemSymbol { name } => {
            icon_theme_pixbuf(system_symbol_icon_name(name))
        }
        NotificationIconAsset::BundleId { bundle_id } => icon_theme_pixbuf(bundle_id),
        NotificationIconAsset::Path { path } => pixbuf_from_path(path),
    }
}

fn pixbuf_from_path(path: &str) -> Option<Pixbuf> {
    let expanded = expand_home(path);
    Pixbuf::from_file_at_scale(&expanded, ICON_SIZE, ICON_SIZE, true).ok()
}

fn expand_home(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/")
        && let Ok(home) = std::env::var("HOME")
    {
        return format!("{home}/{rest}");
    }
    path.to_string()
}

fn default_app_icon() -> Option<Pixbuf> {
    icon_theme_pixbuf("com.hyprnote.dev")
        .or_else(|| icon_theme_pixbuf("anarlog"))
        .or_else(|| icon_theme_pixbuf("hyprnote"))
        .or_else(|| icon_theme_pixbuf("application-x-executable"))
}

fn icon_theme_pixbuf(name: &str) -> Option<Pixbuf> {
    let theme = IconTheme::default()?;
    if let Ok(Some(pixbuf)) = theme.load_icon(name, ICON_SIZE, gtk::IconLookupFlags::FORCE_SIZE) {
        return Some(pixbuf);
    }

    let normalized = name.rsplit('.').next().unwrap_or(name).to_ascii_lowercase();
    theme
        .load_icon(&normalized, ICON_SIZE, gtk::IconLookupFlags::FORCE_SIZE)
        .ok()
        .flatten()
}

fn system_symbol_icon_name(name: &str) -> &str {
    match name {
        "phone.fill" | "phone" => "phone",
        "video.fill" | "video" => "camera-web",
        "calendar" => "x-office-calendar",
        _ => "dialog-information",
    }
}

fn compose_overlay(base: &Pixbuf, badge: &Pixbuf) -> Pixbuf {
    let Some(dest) = base.copy() else {
        return base.clone();
    };

    let badge_size = ((base.width().max(1) as f64) * 0.54).round() as i32;
    let Some(scaled) = badge.scale_simple(badge_size, badge_size, InterpType::Bilinear) else {
        return dest;
    };

    let x = (base.width() - badge_size).max(0);
    let y = (base.height() - badge_size).max(0);
    scaled.composite(
        &dest,
        x,
        y,
        badge_size,
        badge_size,
        f64::from(x),
        f64::from(y),
        1.0,
        1.0,
        InterpType::Bilinear,
        255,
    );
    dest
}

#[cfg(test)]
mod tests {
    use super::expand_home;

    #[test]
    fn expands_home_prefixed_icon_paths() {
        let previous = std::env::var("HOME").ok();
        unsafe {
            std::env::set_var("HOME", "/home/anarlog");
        }
        assert_eq!(
            expand_home("~/icons/zoom.svg"),
            "/home/anarlog/icons/zoom.svg"
        );
        assert_eq!(expand_home("/usr/share/zoom.png"), "/usr/share/zoom.png");
        match previous {
            Some(home) => unsafe { std::env::set_var("HOME", home) },
            None => unsafe { std::env::remove_var("HOME") },
        }
    }
}
