macro_rules! icons {
    ($($name:literal),* $(,)?) => {
        const ICONS: &[(&str, &[u8])] = &[$((
            concat!("sharing/", $name, ".svg"),
            include_bytes!(concat!("icons/", $name, ".svg")),
        )),*];
    };
}

// Regenerate Hugeicons with `node apps/desktop-gpui/src/product/sharing/export-icons.mjs`.
// SlackLogo is the multicolor `logos:slack-icon` that shipping loads through Iconify.
icons!(
    "AddTeamIcon",
    "AlertCircleIcon",
    "ArrowDown01Icon",
    "ArrowLeft01Icon",
    "Building03Icon",
    "CheckIcon",
    "CopyIcon",
    "ExternalLinkIcon",
    "FileTextIcon",
    "Folder01Icon",
    "GlobeIcon",
    "LinkIcon",
    "LoaderCircleIcon",
    "LockKeyIcon",
    "LockPasswordIcon",
    "Login03Icon",
    "Mail02Icon",
    "PaperclipIcon",
    "RefreshIcon",
    "Share08Icon",
    "SlackLogo",
    "TrashIcon",
    "TriangleAlertIcon",
    "UserPlusIcon",
    "UsersIcon",
    "XIcon",
    "ZapIcon",
);

pub fn load(path: &str) -> Option<&'static [u8]> {
    ICONS
        .iter()
        .find(|(name, _)| *name == path)
        .map(|(_, bytes)| *bytes)
}

pub fn paths() -> impl Iterator<Item = &'static str> {
    ICONS.iter().map(|(name, _)| *name)
}
