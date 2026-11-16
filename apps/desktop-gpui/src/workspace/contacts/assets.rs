pub const ICONS: &[(&str, &[u8])] = &[
    (
        "ArrowTurnBackwardIcon",
        include_bytes!("icons/ArrowTurnBackwardIcon.svg"),
    ),
    (
        "ArrowUpDownIcon",
        include_bytes!("icons/ArrowUpDownIcon.svg"),
    ),
    ("Building03Icon", include_bytes!("icons/Building03Icon.svg")),
    ("Camera01Icon", include_bytes!("icons/Camera01Icon.svg")),
    (
        "CircleMinusIcon",
        include_bytes!("icons/CircleMinusIcon.svg"),
    ),
    ("LinkedInIcon", include_bytes!("icons/LinkedInIcon.svg")),
    ("PinIconBold", include_bytes!("icons/PinIconBold.svg")),
    ("PopoverArrow", include_bytes!("icons/PopoverArrow.svg")),
];

pub fn load(path: &str) -> Option<&'static [u8]> {
    let name = path.strip_prefix("contacts/")?.strip_suffix(".svg")?;
    ICONS
        .iter()
        .find(|(key, _)| *key == name)
        .map(|(_, bytes)| *bytes)
}
