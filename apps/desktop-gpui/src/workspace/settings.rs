//! The settings tab: `apps/desktop/src/sidebar/settings.tsx` (the nav that
//! replaces the timeline) and `apps/desktop/src/settings/index.tsx` (the page
//! frame), with the General page from `settings/general`.

use gpui::{
    AnyElement, ClickEvent, Context, Div, MouseButton, SharedString, Stateful, Window, div,
    prelude::*, px, rgb,
};

use super::Workspace;
use crate::text_input::{TextInput, TextInputEvent, TextInputStyle};
use crate::theme::alpha;
use crate::ui::{TailwindText as _, icon};

/// `SettingsTab` values the nav can show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SettingsTab {
    App,
    Account,
    Team,
    Appearance,
    Notifications,
    Transcription,
    Intelligence,
    Dictionary,
    Meetings,
    Sync,
    Imports,
    Privacy,
    Permissions,
    Developers,
}

impl SettingsTab {
    fn title(self) -> &'static str {
        match self {
            Self::App => "General",
            Self::Account => "Account",
            Self::Team => "Teams",
            Self::Appearance => "Appearance",
            Self::Notifications => "Notifications",
            Self::Transcription => "Transcription",
            Self::Intelligence => "Intelligence",
            Self::Dictionary => "Dictionary",
            Self::Meetings => "Meetings",
            Self::Sync => "Sync",
            Self::Imports => "Imports",
            Self::Privacy => "Privacy",
            Self::Permissions => "Permissions",
            Self::Developers => "Developers",
        }
    }
}

enum NavItem {
    Tab {
        tab: SettingsTab,
        label: &'static str,
        icon: &'static str,
        requires_pro: bool,
    },
    /// Items that open another tab type (`destination`), marked with the
    /// arrow; those tabs have no surface in the shell yet.
    Destination {
        label: &'static str,
        icon: &'static str,
        requires_pro: bool,
    },
}

impl NavItem {
    fn label(&self) -> &'static str {
        match self {
            NavItem::Tab { label, .. } | NavItem::Destination { label, .. } => label,
        }
    }
}

/// `groups` in `SettingsNav`; `requiresPro` as the app evaluates it for a
/// signed-out user with no workspaces.
fn nav_groups() -> Vec<(&'static str, Vec<NavItem>)> {
    vec![
        (
            "App",
            vec![
                NavItem::Tab {
                    tab: SettingsTab::App,
                    label: "General",
                    icon: "gear",
                    requires_pro: false,
                },
                NavItem::Tab {
                    tab: SettingsTab::Account,
                    label: "Account",
                    icon: "user",
                    requires_pro: false,
                },
                NavItem::Tab {
                    tab: SettingsTab::Team,
                    label: "Teams",
                    icon: "users-three",
                    requires_pro: true,
                },
                NavItem::Tab {
                    tab: SettingsTab::Appearance,
                    label: "Appearance",
                    icon: "sun",
                    requires_pro: false,
                },
                NavItem::Tab {
                    tab: SettingsTab::Notifications,
                    label: "Notifications",
                    icon: "bell",
                    requires_pro: false,
                },
            ],
        ),
        (
            "AI",
            vec![
                NavItem::Tab {
                    tab: SettingsTab::Transcription,
                    label: "Transcription",
                    icon: "sparkle",
                    requires_pro: false,
                },
                NavItem::Tab {
                    tab: SettingsTab::Intelligence,
                    label: "Intelligence",
                    icon: "sparkle",
                    requires_pro: false,
                },
                NavItem::Tab {
                    tab: SettingsTab::Dictionary,
                    label: "Dictionary",
                    icon: "book-open",
                    requires_pro: true,
                },
            ],
        ),
        (
            "Workspace",
            vec![
                NavItem::Tab {
                    tab: SettingsTab::Meetings,
                    label: "Meetings",
                    icon: "video-camera",
                    requires_pro: false,
                },
                NavItem::Destination {
                    label: "Folders",
                    icon: "folder",
                    requires_pro: false,
                },
                NavItem::Destination {
                    label: "Calendar",
                    icon: "calendar-dots",
                    requires_pro: false,
                },
                NavItem::Destination {
                    label: "Contacts",
                    icon: "users",
                    requires_pro: false,
                },
                NavItem::Destination {
                    label: "Templates",
                    icon: "file-text",
                    requires_pro: false,
                },
                NavItem::Destination {
                    label: "Automations",
                    icon: "lightning",
                    requires_pro: true,
                },
            ],
        ),
        (
            "Data",
            vec![
                NavItem::Tab {
                    tab: SettingsTab::Sync,
                    label: "Sync",
                    icon: "arrows-clockwise",
                    requires_pro: true,
                },
                NavItem::Tab {
                    tab: SettingsTab::Imports,
                    label: "Imports",
                    icon: "download-simple",
                    requires_pro: false,
                },
            ],
        ),
        (
            "Advanced",
            vec![
                NavItem::Tab {
                    tab: SettingsTab::Privacy,
                    label: "Privacy",
                    icon: "shield-check",
                    requires_pro: false,
                },
                NavItem::Tab {
                    tab: SettingsTab::Permissions,
                    label: "Permissions",
                    icon: "lock",
                    requires_pro: false,
                },
                NavItem::Tab {
                    tab: SettingsTab::Developers,
                    label: "Developers",
                    icon: "code",
                    requires_pro: false,
                },
            ],
        ),
    ]
}

/// `--sidebar-accent`
fn sidebar_accent(dark: bool) -> gpui::Rgba {
    if dark { rgb(0x3e3a37) } else { rgb(0xe7e7e4) }
}

/// `font-hand`: "Bradley Hand", "Segoe Print", "Comic Sans MS", cursive; on
/// Linux WebKitGTK ends up on the fontconfig serif, which is what the app
/// shows there.
fn hand_font_family() -> &'static str {
    if cfg!(target_os = "macos") {
        "Bradley Hand"
    } else if cfg!(target_os = "windows") {
        "Segoe Print"
    } else {
        "Noto Serif"
    }
}

/// `SYNCED_SETTING_KEYS` in `settings/schema.ts`.
const SYNCED_SETTING_KEYS: [&str; 6] = [
    "theme",
    "app_icon",
    "sidebar_show_folder",
    "sidebar_show_tags",
    "week_start",
    "default_meeting_share_access",
];

/// Setting keys the General page edits, with their legacy paths and
/// defaults from `settings/schema.ts`.
const AUTOSTART: (&str, &[&str], bool) = ("autostart", &["general", "autostart"], false);
const AUTOMATIC_UPDATES: (&str, &[&str], bool) =
    ("automatic_updates", &["general", "automatic_updates"], true);
const SHOW_TRAY_ICON: (&str, &[&str], bool) =
    ("show_tray_icon", &["general", "show_tray_icon"], true);

fn language_label(code: &str) -> String {
    // `en-US` and friends resolve by their primary subtag.
    let primary = code.split(['-', '_']).next().unwrap_or(code).to_lowercase();
    match primary.as_str() {
        "en" => "English",
        "ko" => "Korean",
        "ja" => "Japanese",
        "zh" => "Chinese",
        "es" => "Spanish",
        "fr" => "French",
        "de" => "German",
        "pt" => "Portuguese",
        "it" => "Italian",
        "nl" => "Dutch",
        "ru" => "Russian",
        "hi" => "Hindi",
        "ar" => "Arabic",
        _ => code,
    }
    .to_string()
}

impl Workspace {
    /// `openNew({ type: "settings", state: { tab } })`
    pub(crate) fn open_settings(
        &mut self,
        tab: SettingsTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.settings_search.is_none() {
            let theme = self.theme;
            let input = cx.new(|cx| {
                TextInput::new(
                    "Search settings...",
                    TextInputStyle {
                        text: theme.foreground,
                        placeholder: theme.muted_foreground,
                        selection: theme.selection,
                        underline_when_focused: false,
                    },
                    window,
                    cx,
                )
            });
            cx.subscribe(&input, |_, _, event: &TextInputEvent, cx| {
                if matches!(event, TextInputEvent::Changed | TextInputEvent::Escape) {
                    cx.notify();
                }
            })
            .detach();
            self.settings_search = Some(input);
        }
        self.settings_tab = Some(tab);
        cx.notify();
    }

    /// `leaveOverlayTab`: back to the tab that was active before.
    pub(crate) fn close_settings(&mut self, cx: &mut Context<Self>) {
        if self.settings_tab.take().is_some() {
            cx.notify();
        }
    }

    pub(crate) fn settings_open(&self) -> bool {
        self.settings_tab.is_some()
    }

    fn set_bool_setting(&mut self, key: &'static str, value: bool, cx: &mut Context<Self>) {
        self.set_setting(key, serde_json::Value::Bool(value), cx);
    }

    /// Optimistically applies a setting, then writes it like `setSettingValues`.
    fn set_setting(&mut self, key: &'static str, value: serde_json::Value, cx: &mut Context<Self>) {
        let synced = SYNCED_SETTING_KEYS.contains(&key);
        self.provider_settings
            .raw
            .insert(key.to_string(), value.to_string());
        if key == "theme" {
            self.theme_preference = value.as_str().unwrap_or("system").to_string();
        }
        cx.notify();
        let task = self.store.set_setting(key.to_string(), value, synced);
        cx.spawn(async move |this, cx| match task.await {
            Ok(Ok(())) => {
                this.update(cx, |this, cx| this.reload_settings(cx)).ok();
            }
            Ok(Err(error)) => tracing::error!(%error, key, "failed to save setting"),
            Err(error) => tracing::error!(%error, key, "failed to save setting"),
        })
        .detach();
    }

    /// `SettingsNav`: back button row, search field, grouped items.
    pub(super) fn render_settings_nav(&self, cx: &Context<Self>) -> Stateful<Div> {
        let theme = self.theme;
        let active = self.settings_tab.unwrap_or(SettingsTab::App);
        let query = self
            .settings_search
            .as_ref()
            .map(|input| input.read(cx).text().trim().to_lowercase())
            .unwrap_or_default();

        let groups: Vec<(&'static str, Vec<NavItem>)> = nav_groups()
            .into_iter()
            .filter_map(|(label, items)| {
                if query.is_empty() || label.to_lowercase().contains(&query) {
                    return Some((label, items));
                }
                let items: Vec<NavItem> = items
                    .into_iter()
                    .filter(|item| item.label().to_lowercase().contains(&query))
                    .collect();
                (!items.is_empty()).then_some((label, items))
            })
            .collect();

        // `CustomSidebarHeader`: `h-12 pt-[9px] pr-1 pl-2` with the `size-7` back button.
        let header = div()
            .flex()
            .h(px(48.0))
            .flex_shrink_0()
            .items_start()
            .pt(px(9.0))
            .pr_1()
            .pl_2()
            .child(
                self.tracked_chrome_button("settings-back", cx)
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_settings(cx)))
                    .child(icon(
                        "arrow-left",
                        px(16.0),
                        self.chrome_icon_color("settings-back"),
                    )),
            );

        let search = div().pb_2().child(
            div()
                .flex()
                .h(px(32.0))
                .w_full()
                .flex_shrink_0()
                .items_center()
                .gap_2()
                .rounded_lg()
                .border_1()
                .border_color(theme.border)
                .bg(alpha(theme.accent, 0.5))
                .px_3()
                .child(icon("search", px(16.0), theme.muted_foreground))
                .children(self.settings_search.clone().map(|input| {
                    div()
                        .flex_1()
                        .min_w_0()
                        .h(px(20.0))
                        .flex()
                        .items_center()
                        .tw_text_sm()
                        .child(input)
                }))
                .when(!query.is_empty(), |field| {
                    field.child(
                        div()
                            .id("settings-search-clear")
                            .size(px(16.0))
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                if let Some(input) = &this.settings_search {
                                    input.update(cx, |input, cx| input.set_text("", cx));
                                }
                                cx.notify();
                            }))
                            .child(icon("x", px(16.0), theme.muted_foreground)),
                    )
                }),
        );

        let accent = sidebar_accent(theme.dark);
        let mut list = div().flex().flex_col().gap(px(20.0)).pb_2();
        if groups.is_empty() {
            list = list.child(
                div()
                    .px_3()
                    .py_8()
                    .flex()
                    .flex_col()
                    .items_center()
                    .text_color(theme.muted_foreground)
                    .child(icon("search", px(32.0), alpha(theme.muted_foreground, 0.7)))
                    .child(div().mt_2().tw_text_sm().child("No results found.")),
            );
        }
        for (label, items) in groups {
            let mut group = div().flex().flex_col().gap(px(2.0)).child(
                div()
                    .px_3()
                    .pb_1()
                    .tw_text_11()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(alpha(theme.muted_foreground, 0.6))
                    .child(SharedString::from(label.to_uppercase())),
            );
            for item in items {
                let (tab, label, glyph, requires_pro, destination) = match item {
                    NavItem::Tab {
                        tab,
                        label,
                        icon,
                        requires_pro,
                    } => (Some(tab), label, icon, requires_pro, false),
                    NavItem::Destination {
                        label,
                        icon,
                        requires_pro,
                    } => (None, label, icon, requires_pro, true),
                };
                let is_active = tab == Some(active);
                let color = if is_active {
                    theme.foreground
                } else {
                    theme.muted_foreground
                };
                group = group.child(
                    div()
                        .id(SharedString::from(format!("settings-nav-{label}")))
                        .flex()
                        .w_full()
                        .items_center()
                        .gap_2()
                        .rounded_full()
                        .px_3()
                        .py_2()
                        .tw_text_sm()
                        .text_color(color)
                        .cursor_pointer()
                        .when(is_active, |item| {
                            item.bg(accent).font_weight(gpui::FontWeight::MEDIUM)
                        })
                        .when(!is_active, |item| {
                            item.hover(move |style| {
                                style.bg(alpha(accent, 0.5)).text_color(theme.foreground)
                            })
                        })
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            if let Some(tab) = tab {
                                this.settings_tab = Some(tab);
                                cx.notify();
                            }
                        }))
                        .child(icon(glyph, px(15.0), color))
                        .child(
                            div()
                                .flex()
                                .min_w_0()
                                .flex_1()
                                .items_center()
                                .gap_2()
                                .child(div().min_w_0().flex_1().truncate().child(label))
                                .when(requires_pro, |row| row.child(icon("lock", px(14.0), color)))
                                .when(!requires_pro && destination, |row| {
                                    row.child(icon(
                                        "arrow-up-right",
                                        px(14.0),
                                        alpha(theme.muted_foreground, 0.7),
                                    ))
                                }),
                        ),
                );
            }
            list = list.child(group);
        }

        // The sidebar in a special mode: `gap-1 pr-1`, the nav's own header.
        div()
            .id("settings-nav")
            .flex()
            .flex_col()
            .h_full()
            .w(px(self.sidebar_width))
            .flex_shrink_0()
            .pr_1()
            .overflow_hidden()
            .child(header)
            .child(search)
            .child(
                div()
                    .id("settings-nav-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(list),
            )
    }

    /// `SettingsView`: `bg-card dark:bg-accent`, content `px-6 pt-6 pb-10`.
    pub(super) fn render_settings_content(&self, cx: &Context<Self>) -> Stateful<Div> {
        let theme = self.theme;
        let tab = self.settings_tab.unwrap_or(SettingsTab::App);
        let title = div()
            .font_family(hand_font_family())
            .text_size(px(30.0))
            .line_height(px(37.5))
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(theme.foreground)
            .child(tab.title());

        let page = match tab {
            SettingsTab::App => div()
                .flex()
                .flex_col()
                .gap_8()
                .child(title)
                .child(self.render_general_settings(cx)),
            // `SettingsAppearance`: `max-w-5xl gap-10`.
            SettingsTab::Appearance => div()
                .flex()
                .flex_col()
                .max_w(px(1024.0))
                .gap_10()
                .child(title)
                .child(self.render_appearance_settings(cx)),
            _ => div().flex().flex_col().gap_8().child(title),
        };

        div()
            .id("settings-content")
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .h_full()
            .bg(if theme.dark { theme.accent } else { theme.card })
            .overflow_y_scroll()
            .px_6()
            .pt_6()
            .pb_10()
            .child(page)
    }

    /// `AppSettingsView` + Language & Region + Storage.
    fn render_general_settings(&self, cx: &Context<Self>) -> Div {
        let theme = self.theme;
        let settings = &self.provider_settings;
        let autostart = settings.bool_setting(AUTOSTART.0, AUTOSTART.1, AUTOSTART.2);
        let updates = settings.bool_setting(
            AUTOMATIC_UPDATES.0,
            AUTOMATIC_UPDATES.1,
            AUTOMATIC_UPDATES.2,
        );
        let tray = settings.bool_setting(SHOW_TRAY_ICON.0, SHOW_TRAY_ICON.1, SHOW_TRAY_ICON.2);
        let language = settings
            .string_setting("ai_language", &["general", "ai_language"])
            .unwrap_or_else(|| "en".to_string());
        let timezone = settings.string_setting("timezone", &["general", "timezone"]);
        let week_start = settings
            .string_setting("week_start", &["general", "week_start"])
            .unwrap_or_else(|| "sunday".to_string());
        let vault = self
            .store
            .path()
            .parent()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        let home = dirs::home_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        let vault_display = if !home.is_empty() && vault.starts_with(&home) {
            format!("~{}", &vault[home.len()..])
        } else {
            vault
        };

        let switch_row = |id: &'static str,
                          title: &'static str,
                          description: Option<&'static str>,
                          checked: bool,
                          key: &'static str| {
            setting_row(
                theme,
                title,
                description,
                false,
                render_switch(
                    theme,
                    id,
                    checked,
                    cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.set_bool_setting(key, !checked, cx);
                    }),
                )
                .into_any_element(),
            )
        };

        div()
            .flex()
            .flex_col()
            .gap_8()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(switch_row(
                        "setting-autostart",
                        "Start Anarlog at login",
                        Some("Have Anarlog ready when you sign in."),
                        autostart,
                        AUTOSTART.0,
                    ))
                    .child(switch_row(
                        "setting-updates",
                        "Automatically install updates",
                        Some("Stay current with updates installed the next time Anarlog opens."),
                        updates,
                        AUTOMATIC_UPDATES.0,
                    ))
                    .child(switch_row(
                        "setting-tray",
                        "Show tray icon",
                        None,
                        tray,
                        SHOW_TRAY_ICON.0,
                    )),
            )
            .child(
                div()
                    .child(section_heading(theme, "Language & Region"))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_6()
                            .child(setting_row(
                                theme,
                                "Main language",
                                Some("Use this language for summaries and AI responses."),
                                true,
                                render_select(
                                    theme,
                                    Some(language_label(&language)),
                                    "Select language",
                                ),
                            ))
                            .child(setting_row(
                                theme,
                                "Timezone",
                                Some("Show the timeline in your preferred timezone."),
                                true,
                                render_select(theme, timezone, "Select timezone"),
                            ))
                            .child(setting_row(
                                theme,
                                "Week starts on",
                                Some("Choose which day begins your calendar week."),
                                true,
                                render_select(
                                    theme,
                                    Some(
                                        if week_start == "monday" {
                                            "Monday"
                                        } else {
                                            "Sunday"
                                        }
                                        .to_string(),
                                    ),
                                    "Select day",
                                ),
                            ))
                            .child(setting_row(
                                theme,
                                "Additional spoken languages",
                                Some("Transcribe meetings that use more than one language."),
                                true,
                                render_select(theme, None, "Add languages"),
                            )),
                    ),
            )
            .child(
                div().child(section_heading(theme, "Storage")).child(
                    // `StorageLocationRow`: `grid-cols-[minmax(0,1fr)_9rem] gap-3`.
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(
                            div()
                                .flex()
                                .min_w_0()
                                .flex_1()
                                .items_center()
                                .gap_2()
                                .rounded_lg()
                                .px_2()
                                .py_2()
                                .child(icon("folder", px(16.0), theme.muted_foreground))
                                .child(
                                    div()
                                        .min_w_0()
                                        .child(
                                            div()
                                                .tw_text_sm()
                                                .font_weight(gpui::FontWeight::MEDIUM)
                                                .child(
                                                    "Where your notes and recordings are stored",
                                                ),
                                        )
                                        .child(
                                            div()
                                                .tw_text_xs()
                                                .text_color(theme.muted_foreground)
                                                .truncate()
                                                .child(SharedString::from(vault_display)),
                                        ),
                                ),
                        )
                        .child(
                            // `Button variant="outline" className="h-9 w-full"` in a 9rem column.
                            div()
                                .w(px(144.0))
                                .h(px(36.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_full()
                                .border_1()
                                .border_color(theme.border)
                                .bg(theme.background)
                                .tw_text_sm()
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .child("Change"),
                        ),
                ),
            )
    }
}

impl Workspace {
    /// `ThemeSelector` + `SidebarItemFieldsSettings` (the app icon picker is
    /// macOS-only).
    fn render_appearance_settings(&self, cx: &Context<Self>) -> Div {
        let theme = self.theme;
        let settings = &self.provider_settings;
        let current = self.theme_preference.clone();
        let show_folder = settings.bool_setting(
            "sidebar_show_folder",
            &["general", "sidebar_show_folder"],
            true,
        );
        let show_tags = settings.bool_setting(
            "sidebar_show_tags",
            &["general", "sidebar_show_tags"],
            false,
        );

        let options: [(&str, &str, &str); 3] = [
            ("light", "Light", "Bright canvas"),
            ("dark", "Dark", "Low-light canvas"),
            ("system", "System", "Match your device"),
        ];
        let cards = options.into_iter().map(|(value, label, description)| {
            let selected = current == value;
            div()
                .id(SharedString::from(format!("theme-{value}")))
                .relative()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .rounded_2xl()
                .border_1()
                .border_color(if selected {
                    alpha(theme.foreground, 0.5)
                } else {
                    theme.border
                })
                .bg(if selected {
                    alpha(theme.accent, 0.4)
                } else {
                    theme.background
                })
                .when(selected, |card| card.shadow_xs())
                .when(!selected, |card| {
                    card.hover(move |style| {
                        style
                            .border_color(alpha(theme.foreground, 0.3))
                            .bg(alpha(theme.accent, 0.2))
                    })
                })
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.set_setting("theme", serde_json::Value::String(value.to_string()), cx);
                }))
                .child(theme_preview(value))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .border_t_1()
                        .border_color(theme.border)
                        .p_3()
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .child(
                                    div()
                                        .tw_text_sm()
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .text_color(theme.foreground)
                                        .child(label),
                                )
                                .child(
                                    div()
                                        .tw_text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child(description),
                                ),
                        )
                        .when(selected, |row| {
                            row.child(
                                div()
                                    .size(px(20.0))
                                    .flex_shrink_0()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_full()
                                    .bg(theme.foreground)
                                    .child(icon("check", px(12.0), theme.background)),
                            )
                        }),
                )
        });

        div()
            .flex()
            .flex_col()
            .gap_10()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div()
                            .child(
                                div()
                                    .tw_text_lg()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(theme.foreground)
                                    .child("Theme"),
                            )
                            .child(
                                div()
                                    .mt_1()
                                    .tw_text_sm()
                                    .text_color(theme.muted_foreground)
                                    .child("Choose how Anarlog looks on this device."),
                            ),
                    )
                    .child(div().flex().gap_3().children(cards)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div()
                            .child(
                                div()
                                    .tw_text_lg()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(theme.foreground)
                                    .child("Notes list"),
                            )
                            .child(
                                div()
                                    .mt_1()
                                    .tw_text_sm()
                                    .text_color(theme.muted_foreground)
                                    .child("Choose extra fields to show on each note."),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_4()
                            .child(setting_row(
                                theme,
                                "Folder",
                                Some("Show the folder above the title."),
                                false,
                                render_switch(
                                    theme,
                                    "setting-show-folder",
                                    show_folder,
                                    cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        this.set_bool_setting(
                                            "sidebar_show_folder",
                                            !show_folder,
                                            cx,
                                        );
                                    }),
                                )
                                .into_any_element(),
                            ))
                            .child(setting_row(
                                theme,
                                "Tags",
                                Some("Show tags under the date and time."),
                                false,
                                render_switch(
                                    theme,
                                    "setting-show-tags",
                                    show_tags,
                                    cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        this.set_bool_setting("sidebar_show_tags", !show_tags, cx);
                                    }),
                                )
                                .into_any_element(),
                            )),
                    ),
            )
    }
}

/// `ThemePreview`: a `h-28` mock canvas; the system card is the light canvas
/// with the dark one clipped to the lower-left triangle.
fn theme_preview(value: &str) -> Div {
    let dark = value == "dark";
    let mut preview = div()
        .relative()
        .h(px(112.0))
        .overflow_hidden()
        .child(preview_canvas(dark));
    if value == "system" {
        // No clip-path in GPUI: approximate the diagonal with a dark canvas
        // shifted so it fills the lower-left half.
        preview = preview.child(
            div()
                .absolute()
                .top_0()
                .left(px(-140.0))
                .bottom_0()
                .w(px(280.0))
                .child(preview_canvas(true)),
        );
    }
    preview
}

fn preview_canvas(dark: bool) -> Div {
    let (bg, side, title, line) = if dark {
        (rgb(0x0a0a0a), rgb(0x262626), rgb(0xd4d4d4), rgb(0x404040))
    } else {
        (rgb(0xffffff), rgb(0xf5f5f5), rgb(0x404040), rgb(0xe5e5e5))
    };
    div()
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .bottom_0()
        .flex()
        .flex_col()
        .p_3()
        .bg(bg)
        .child(
            div()
                .mb_3()
                .flex()
                .gap_1()
                .child(div().size(px(6.0)).rounded_full().bg(rgb(0xf87171)))
                .child(div().size(px(6.0)).rounded_full().bg(rgb(0xfbbf24)))
                .child(div().size(px(6.0)).rounded_full().bg(rgb(0x4ade80))),
        )
        .child(
            div()
                .flex()
                .flex_1()
                .gap_3()
                .child(div().w(px(46.0)).rounded_md().bg(side))
                .child(
                    div()
                        .flex()
                        .flex_1()
                        .flex_col()
                        .gap_2()
                        .py_1()
                        .child(div().h(px(6.0)).w(px(40.0)).rounded_full().bg(title))
                        .child(div().h(px(4.0)).w_4_5().rounded_full().bg(line))
                        .child(div().h(px(4.0)).w_3_5().rounded_full().bg(line))
                        .child(div().h(px(4.0)).w_2_3().rounded_full().bg(line)),
                ),
        )
}

/// `<h2 className="mb-4 font-sans text-lg font-semibold">`
fn section_heading(theme: crate::theme::Theme, label: &'static str) -> Div {
    div()
        .mb_4()
        .tw_text_lg()
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(theme.foreground)
        .child(label)
}

/// `SettingRow`: title `text-sm font-medium mb-1`, description `text-xs
/// text-muted-foreground`, control in a `w-48` column (or content width).
fn setting_row(
    theme: crate::theme::Theme,
    title: &'static str,
    description: Option<&'static str>,
    fixed_control: bool,
    control: AnyElement,
) -> Div {
    div()
        .flex()
        .w_full()
        .min_w_0()
        .items_center()
        .justify_between()
        .gap_4()
        .child(
            div()
                .min_w_0()
                .flex_1()
                .child(
                    div()
                        .mb_1()
                        .tw_text_sm()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(theme.foreground)
                        .child(title),
                )
                .when_some(description, |column, description| {
                    column.child(
                        div()
                            .tw_text_xs()
                            .text_color(theme.muted_foreground)
                            .child(description),
                    )
                }),
        )
        .child(
            div()
                .flex()
                .justify_end()
                .when(fixed_control, |column| column.w(px(192.0)).min_w_0())
                .when(!fixed_control, |column| column.flex_shrink_0())
                .child(control),
        )
}

/// `Switch` (default size): `h-6 w-11 rounded-pill border-2`, `bg-muted`
/// unchecked / `bg-primary border-primary` checked, `h-5 w-5` thumb sliding
/// 20px, `bg-background` (`bg-primary-foreground` when checked).
fn render_switch(
    theme: crate::theme::Theme,
    id: &'static str,
    checked: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut gpui::App) + 'static,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .h(px(24.0))
        .w(px(44.0))
        .rounded_full()
        .border_2()
        .border_color(if checked { theme.primary } else { theme.border })
        .bg(if checked { theme.primary } else { theme.muted })
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(on_click)
        .child(
            div()
                .size(px(20.0))
                .rounded_full()
                .ml(if checked { px(20.0) } else { px(0.0) })
                .bg(if checked {
                    theme.primary_foreground
                } else {
                    theme.background
                })
                .shadow_lg(),
        )
}

/// `SelectTrigger` with `SETTING_CONTROL_CLASS`: `h-9 w-full rounded-full
/// border bg-card px-3 text-sm` with the half-opacity caret.
fn render_select(
    theme: crate::theme::Theme,
    value: Option<String>,
    placeholder: &'static str,
) -> AnyElement {
    let (text, color) = match value {
        Some(value) => (SharedString::from(value), theme.foreground),
        None => (SharedString::from(placeholder), theme.muted_foreground),
    };
    div()
        .flex()
        .h(px(36.0))
        .w_full()
        .items_center()
        .justify_between()
        .rounded_full()
        .border_1()
        .border_color(theme.border)
        .bg(theme.card)
        .px_3()
        .py_2()
        .tw_text_sm()
        .text_color(color)
        .child(div().min_w_0().truncate().child(text))
        .child(icon("caret-down", px(16.0), alpha(theme.foreground, 0.5)))
        .into_any_element()
}
