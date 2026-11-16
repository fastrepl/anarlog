use std::borrow::Cow;

use gpui::{AssetSource, SharedString};

pub struct Assets;

const ICONS: &[(&str, &[u8])] = &[
    (
        "ArrowLeft02Icon",
        include_bytes!("icons/ArrowLeft02Icon.svg"),
    ),
    (
        "ArrowRight02Icon",
        include_bytes!("icons/ArrowRight02Icon.svg"),
    ),
    (
        "SidebarLeftIcon",
        include_bytes!("icons/SidebarLeftIcon.svg"),
    ),
    ("PinIcon", include_bytes!("icons/PinIcon.svg")),
    ("UsersIcon", include_bytes!("icons/UsersIcon.svg")),
    ("Folder01Icon", include_bytes!("icons/Folder01Icon.svg")),
    ("Calendar03Icon", include_bytes!("icons/Calendar03Icon.svg")),
    ("ZapIcon", include_bytes!("icons/ZapIcon.svg")),
    ("FileTextIcon", include_bytes!("icons/FileTextIcon.svg")),
];

impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        if let Some(name) = path
            .strip_prefix("workspace/")
            .and_then(|p| p.strip_suffix(".svg"))
            && let Some((_, bytes)) = ICONS.iter().find(|(key, _)| *key == name)
        {
            return Ok(Some(Cow::Borrowed(bytes)));
        }
        crate::ui::assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        let mut paths = crate::ui::assets::Assets.list(path)?;
        paths.extend(
            ICONS
                .iter()
                .map(|(name, _)| format!("workspace/{name}.svg").into()),
        );
        Ok(paths)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_source_icons_and_existing_assets_are_resolvable() {
        for path in Assets.list("").unwrap() {
            let bytes = Assets.load(path.as_ref()).unwrap().unwrap();
            assert!(std::str::from_utf8(&bytes).unwrap().contains("<svg"));
        }
        assert!(Assets.load("workspace/missing.svg").unwrap().is_none());
    }
}
