use std::borrow::Cow;

use gpui::{AssetSource, SharedString};

/// Icons are the Hugeicons (MIT) glyphs the Tauri app maps in
/// `packages/ui/src/components/icons.tsx`; brand images come from
/// `apps/desktop/public/assets`. Embedded so the binary is self-contained.
pub struct Assets;

macro_rules! embedded {
    ($($path:literal),* $(,)?) => {
        const FILES: &[(&str, &[u8])] = &[
            $(($path, include_bytes!(concat!("../assets/", $path))),)*
        ];
    };
}

embedded!(
    "icons/app-window.svg",
    "icons/arrow-down.svg",
    "icons/arrow-left.svg",
    "icons/arrow-up-right.svg",
    "icons/arrow-up.svg",
    "icons/arrows-clockwise.svg",
    "icons/bell.svg",
    "icons/book-open.svg",
    "icons/calendar-blank.svg",
    "icons/calendar-dots.svg",
    "icons/calendar.svg",
    "icons/caret-down.svg",
    "icons/caret-right.svg",
    "icons/check.svg",
    "icons/close-x.svg",
    "icons/code.svg",
    "icons/download-simple.svg",
    "icons/file-arrow-down.svg",
    "icons/file-text.svg",
    "icons/filter.svg",
    "icons/folder-open.svg",
    "icons/folder.svg",
    "icons/gear.svg",
    "icons/headset.svg",
    "icons/lightning.svg",
    "icons/lock.svg",
    "icons/microphone.svg",
    "icons/more-horizontal.svg",
    "icons/note-edit.svg",
    "icons/popover-tail-border.svg",
    "icons/popover-tail.svg",
    "icons/search.svg",
    "icons/shield-check.svg",
    "icons/sidebar-left.svg",
    "icons/sort-ascending.svg",
    "icons/sort-descending.svg",
    "icons/sparkle.svg",
    "icons/square.svg",
    "icons/sun.svg",
    "icons/text-align-left.svg",
    "icons/trash.svg",
    "icons/user.svg",
    "icons/users-three.svg",
    "icons/users.svg",
    "icons/video-camera.svg",
    "icons/view-sidebar-left.svg",
    "icons/x.svg",
    "anarlog-icon.png",
    "google-meet.svg",
    "teams.png",
    "webex.png",
    "zoom-icon.svg",
);

impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        Ok(FILES
            .iter()
            .find(|(name, _)| *name == path)
            .map(|(_, bytes)| Cow::Borrowed(*bytes)))
    }

    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok(FILES
            .iter()
            .filter(|(name, _)| name.starts_with(path))
            .map(|(name, _)| SharedString::from(*name))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_embedded_icon_is_an_svg_document() {
        for (name, bytes) in FILES.iter().filter(|(name, _)| name.ends_with(".svg")) {
            let text = std::str::from_utf8(bytes).unwrap();
            assert!(text.starts_with("<svg"), "{name} is not an svg");
        }
        assert!(Assets.load("icons/search.svg").unwrap().is_some());
        assert!(Assets.load("icons/missing.svg").unwrap().is_none());
        assert_eq!(Assets.list("icons/").unwrap().len(), 47);
    }
}
