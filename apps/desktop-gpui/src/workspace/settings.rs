//! The settings tab: `apps/desktop/src/sidebar/settings.tsx` (the nav that
//! replaces the timeline) and `apps/desktop/src/settings/index.tsx` (the page
//! frame), with the General page from `settings/general`.

use std::rc::Rc;

use gpui::{
    AnyElement, ClickEvent, Context, Div, Focusable as _, MouseButton, SharedString, Stateful,
    Window, div, prelude::*, px, rgb,
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
    Stats,
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
            Self::Stats => "Your stats",
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
                    tab: SettingsTab::Stats,
                    label: "Stats",
                    icon: "chart-bar",
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
pub(super) fn hand_font_family() -> &'static str {
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

impl Workspace {
    /// `openNew({ type: "settings", state: { tab } })`
    pub(crate) fn open_settings(
        &mut self,
        tab: SettingsTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // `openNew({ type: "settings" })` replaces the other overlay tabs.
        self.close_folders(cx);
        self.close_templates(cx);
        self.close_calendar(cx);
        self.close_contacts(cx);
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
                        masked: false,
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
        self.ensure_dictionary_input(window, cx);
        if self.spoken_search.is_none() {
            let theme = self.theme;
            let input = cx.new(|cx| {
                TextInput::new(
                    "Add language",
                    TextInputStyle {
                        text: theme.foreground,
                        placeholder: theme.muted_foreground,
                        selection: theme.selection,
                        underline_when_focused: false,
                        masked: false,
                    },
                    window,
                    cx,
                )
            });
            cx.subscribe_in(
                &input,
                window,
                |this, input, event: &TextInputEvent, window, cx| match event {
                    TextInputEvent::Changed => {
                        this.spoken_highlighted = None;
                        cx.notify();
                    }
                    TextInputEvent::Navigate(delta) => {
                        let count = this.spoken_language_matches(cx).len();
                        if count == 0 {
                            return;
                        }
                        let current = this.spoken_highlighted.map(|i| i as i32).unwrap_or(-1);
                        let next = (current + delta).clamp(0, count as i32 - 1);
                        this.spoken_highlighted = Some(next as usize);
                        cx.notify();
                    }
                    TextInputEvent::Enter => {
                        let matches = this.spoken_language_matches(cx);
                        if let Some(code) = this
                            .spoken_highlighted
                            .and_then(|i| matches.get(i).cloned())
                        {
                            this.add_spoken_language(&code, cx);
                            input.update(cx, |input, cx| input.set_text("", cx));
                            this.spoken_highlighted = None;
                        }
                        // Keep typing: the web input stays focused on Enter.
                        input.read(cx).focus_handle(cx).focus(window);
                    }
                    TextInputEvent::BackspaceEmpty => {
                        let mut spoken = this.spoken_languages();
                        if spoken.pop().is_some() {
                            this.write_spoken_languages(spoken, cx);
                        }
                    }
                    TextInputEvent::Escape => {
                        input.update(cx, |input, cx| input.set_text("", cx));
                        this.focus_handle.focus(window);
                        cx.notify();
                    }
                    TextInputEvent::Committed => cx.notify(),
                },
            )
            .detach();
            self.spoken_search = Some(input);
        }
        match tab {
            SettingsTab::Transcription => {
                self.ensure_ai_settings(super::ai_settings::ProviderKind::Stt, window, cx)
            }
            SettingsTab::Intelligence => {
                self.ensure_ai_settings(super::ai_settings::ProviderKind::Llm, window, cx)
            }
            SettingsTab::Permissions => {
                for permission in ["microphone", "system_audio"] {
                    self.check_permission(permission, cx);
                }
            }
            SettingsTab::Developers => self.ensure_developers(window, cx),
            SettingsTab::Stats => self.ensure_stats(cx),
            _ => {}
        }
        self.settings_tab = Some(tab);
        cx.notify();
    }

    /// `getAdditionalSpokenLanguages(ai_language, spoken_languages)`
    pub(super) fn spoken_languages(&self) -> Vec<String> {
        let main = self
            .provider_settings
            .string_setting("ai_language", &["language", "ai_language"])
            .unwrap_or_else(|| "en".to_string());
        let spoken = self
            .provider_settings
            .string_setting("spoken_languages", &["language", "spoken_languages"])
            .and_then(|json| serde_json::from_str::<Vec<String>>(&json).ok())
            .unwrap_or_default();
        additional_spoken_languages(&main, &spoken)
    }

    fn write_spoken_languages(&mut self, spoken: Vec<String>, cx: &mut Context<Self>) {
        self.set_setting(
            "spoken_languages",
            serde_json::Value::String(
                serde_json::to_string(&spoken).unwrap_or_else(|_| "[]".to_string()),
            ),
            cx,
        );
    }

    fn add_spoken_language(&mut self, code: &str, cx: &mut Context<Self>) {
        let mut spoken = self.spoken_languages();
        spoken.push(code.to_string());
        self.write_spoken_languages(spoken, cx);
    }

    /// `filteredLanguages`: supported codes minus the main and chosen ones,
    /// matched by display name; empty until something is typed.
    fn spoken_language_matches(&self, cx: &Context<Self>) -> Vec<String> {
        let query = self
            .spoken_search
            .as_ref()
            .map(|input| input.read(cx).text().trim().to_lowercase())
            .unwrap_or_default();
        if query.is_empty() {
            return Vec::new();
        }
        let main = base_language_code(
            &self
                .provider_settings
                .string_setting("ai_language", &["language", "ai_language"])
                .unwrap_or_else(|| "en".to_string()),
        );
        let chosen = self.spoken_languages();
        CORE_LANGUAGES
            .iter()
            .filter(|(code, label)| {
                *code != main
                    && !chosen.iter().any(|c| c == code)
                    && label.to_lowercase().contains(&query)
            })
            .map(|(code, _)| code.to_string())
            .collect()
    }

    /// `SpokenLanguagesView`: heading, description, and the chip input with
    /// its `top-full mt-1` results list while typing.
    fn render_spoken_languages(&self, window: &Window, cx: &Context<Self>) -> Div {
        let theme = self.theme;
        let chosen = self.spoken_languages();
        let focused = self
            .spoken_search
            .as_ref()
            .is_some_and(|input| input.read(cx).focus_handle(cx).is_focused(window));
        let query_present = self
            .spoken_search
            .as_ref()
            .is_some_and(|input| !input.read(cx).text().trim().is_empty());
        let matches = self.spoken_language_matches(cx);

        let mut field = div()
            .id("spoken-languages-field")
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(6.0))
            .min_h(px(38.0))
            .w_full()
            .rounded(px(16.0))
            .border_1()
            .border_color(theme.border)
            .bg(theme.card)
            .px_2()
            .py(px(6.0))
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                if let Some(input) = &this.spoken_search {
                    input.read(cx).focus_handle(cx).focus(window);
                }
            }));
        for code in &chosen {
            let label = CORE_LANGUAGES
                .iter()
                .find(|(c, _)| c == code)
                .map(|(_, label)| label.to_string())
                .unwrap_or_else(|| code.clone());
            let code_for_click = code.clone();
            field = field.child(
                // `Badge variant="secondary"` with `bg-muted px-2 py-0.5 text-xs`.
                div()
                    .id(SharedString::from(format!("spoken-{code}")))
                    .relative()
                    .flex()
                    .items_center()
                    .gap_1()
                    // `Badge`: the control squircle.
                    .child(crate::squircle::squircle(
                        crate::squircle::CONTROL_RADIUS,
                        Some(theme.muted),
                        None,
                    ))
                    .px_2()
                    .py(px(2.0))
                    .tw_text_xs()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme.foreground)
                    .child(SharedString::from(label))
                    .child(
                        div()
                            .id(SharedString::from(format!("spoken-remove-{code}")))
                            .ml(px(2.0))
                            .size(px(12.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                let spoken = this
                                    .spoken_languages()
                                    .into_iter()
                                    .filter(|c| *c != code_for_click)
                                    .collect();
                                this.write_spoken_languages(spoken, cx);
                            }))
                            .child(icon("x", px(10.0), theme.foreground)),
                    ),
            );
        }
        if chosen.is_empty() {
            field = field.child(icon("search", px(16.0), theme.muted_foreground));
        }
        if let Some(input) = self.spoken_search.clone() {
            field = field.child(div().flex_1().min_w(px(120.0)).tw_text_sm().child(input));
        }

        div()
            .child(
                div()
                    .mb_1()
                    .tw_text_sm()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.foreground)
                    .child("Additional spoken languages"),
            )
            .child(
                div()
                    .mb_3()
                    .tw_text_xs()
                    .text_color(theme.muted_foreground)
                    .child("Transcribe meetings that use more than one language."),
            )
            .child(
                div()
                    .relative()
                    .child(field)
                    .when(focused && query_present, |wrapper| {
                        let mut list = div()
                            .id("spoken-languages-options")
                            .occlude()
                            .absolute()
                            .top(px(38.0))
                            .left_0()
                            .right_0()
                            .mt_1()
                            .flex()
                            .flex_col()
                            .max_h(px(240.0))
                            .overflow_y_scroll()
                            .rounded(px(16.0))
                            .border_1()
                            .border_color(theme.border)
                            .bg(theme.card)
                            .shadow_md();
                        if matches.is_empty() {
                            list = list.child(
                                div()
                                    .px_3()
                                    .py_2()
                                    .text_center()
                                    .tw_text_sm()
                                    .text_color(theme.muted_foreground)
                                    .child("No matching languages found"),
                            );
                        } else {
                            list = list.children(matches.into_iter().enumerate().map(
                                |(index, code)| {
                                    let label = CORE_LANGUAGES
                                        .iter()
                                        .find(|(c, _)| *c == code)
                                        .map(|(_, label)| label.to_string())
                                        .unwrap_or_else(|| code.clone());
                                    let highlighted = self.spoken_highlighted == Some(index);
                                    div()
                                        .id(SharedString::from(format!("spoken-option-{index}")))
                                        .flex()
                                        .w_full()
                                        .items_center()
                                        .justify_between()
                                        .px_3()
                                        .py_2()
                                        .tw_text_sm()
                                        .text_color(theme.foreground)
                                        .when(highlighted, |row| row.bg(theme.accent))
                                        .hover(move |style| style.bg(theme.accent))
                                        .on_hover(cx.listener(
                                            move |this, hovered: &bool, _, cx| {
                                                if *hovered
                                                    && this.spoken_highlighted != Some(index)
                                                {
                                                    this.spoken_highlighted = Some(index);
                                                    cx.notify();
                                                }
                                            },
                                        ))
                                        // `onMouseDown={(e) => e.preventDefault()}` keeps the input focused.
                                        .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                            cx.stop_propagation()
                                        })
                                        .on_click(cx.listener(
                                            move |this, _: &ClickEvent, window, cx| {
                                                this.add_spoken_language(&code, cx);
                                                if let Some(input) = &this.spoken_search {
                                                    input.update(cx, |input, cx| {
                                                        input.set_text("", cx)
                                                    });
                                                    input.read(cx).focus_handle(cx).focus(window);
                                                }
                                                this.spoken_highlighted = None;
                                            },
                                        ))
                                        .child(
                                            div()
                                                .truncate()
                                                .font_weight(gpui::FontWeight::MEDIUM)
                                                .child(SharedString::from(label)),
                                        )
                                },
                            ));
                        }
                        wrapper.child(gpui::deferred(list).with_priority(1))
                    }),
            )
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
    pub(super) fn set_setting(
        &mut self,
        key: &'static str,
        value: serde_json::Value,
        cx: &mut Context<Self>,
    ) {
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
                        // `.rounded-full` is `0.5rem` in the desktop app.
                        .rounded(px(8.0))
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
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            if let Some(tab) = tab {
                                this.open_settings(tab, window, cx);
                            } else if label == "Folders" {
                                this.open_folders(window, cx);
                            } else if label == "Templates" {
                                this.open_templates(None, window, cx);
                            } else if label == "Calendar" {
                                this.open_calendar(cx);
                            } else if label == "Contacts" {
                                this.open_contacts(window, cx);
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
            .w(px(self.custom_sidebar_width()))
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
    pub(super) fn render_settings_content(
        &self,
        window: &Window,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let theme = self.theme;
        let tab = self.settings_tab.unwrap_or(SettingsTab::App);
        let title = div()
            .font_family(hand_font_family())
            .text_size(px(30.0))
            .line_height(px(37.0))
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(theme.foreground)
            .child(tab.title());

        let page = match tab {
            SettingsTab::App => div()
                .flex()
                .flex_col()
                .gap_8()
                .child(title)
                .child(self.render_general_settings(window, cx)),
            // `SettingsAppearance`: `max-w-5xl gap-10`.
            SettingsTab::Appearance => div()
                .flex()
                .flex_col()
                .max_w(px(1024.0))
                .gap_10()
                .child(title)
                .child(self.render_appearance_settings(cx)),
            // `SettingsNotifications`: `gap-6` between the title and the rows.
            SettingsTab::Notifications => div()
                .flex()
                .flex_col()
                .gap_6()
                .child(title)
                .child(self.render_notification_settings(cx)),
            SettingsTab::Transcription => {
                self.render_ai_settings(super::ai_settings::ProviderKind::Stt, title, window, cx)
            }
            SettingsTab::Intelligence => {
                self.render_ai_settings(super::ai_settings::ProviderKind::Llm, title, window, cx)
            }
            SettingsTab::Developers => self.render_developers_settings(title, cx),
            SettingsTab::Dictionary => self.render_dictionary_settings(title, window, cx),
            // `SettingsTeam` signed out: the title and one muted line.
            SettingsTab::Team => div().flex().flex_col().gap_8().child(title).child(
                div()
                    .tw_text_sm()
                    .text_color(theme.muted_foreground)
                    .child("Sign in to create a shared workspace for your team."),
            ),
            // `SettingsSync` while `settingsQuery` is loading (no API without
            // an account): the `min-h-48` centred spinner, no title.
            SettingsTab::Sync => div()
                .flex()
                .min_h(px(192.0))
                .items_center()
                .justify_center()
                .child(crate::ui::spinner(
                    "sync-loading",
                    px(20.0),
                    theme.muted_foreground,
                )),
            // `SettingsImports`: the title row with the ghost Documentation
            // button, then `MeetingImportScreen` (full width).
            SettingsTab::Imports => div()
                .flex()
                .flex_col()
                .gap_8()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_4()
                        .child(title)
                        .child(
                            // `Button variant="ghost" size="sm"`: `h-7 px-2 gap-2 text-xs`.
                            div()
                                .id("imports-documentation")
                                .flex()
                                .h(px(28.0))
                                .items_center()
                                .gap_2()
                                .px_2()
                                .rounded(px(8.0))
                                .tw_text_xs()
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .text_color(theme.foreground)
                                .cursor_pointer()
                                .hover(|s| s.bg(theme.accent))
                                .on_click(cx.listener(|_, _: &gpui::ClickEvent, _, cx| {
                                    cx.open_url("https://docs.anarlog.so/imports");
                                }))
                                .child("Documentation")
                                .child(crate::ui::icon(
                                    "arrow-square-out",
                                    px(14.0),
                                    theme.foreground,
                                )),
                        ),
                )
                .child(self.render_meeting_import_card(false, window, cx)),
            SettingsTab::Account => div()
                .flex()
                .flex_col()
                .gap_8()
                .child(title)
                .child(self.render_account_signed_out(cx))
                .child(self.render_guest_plans()),
            SettingsTab::Stats => self.render_stats_settings(title, window, cx),
            SettingsTab::Permissions => div()
                .flex()
                .flex_col()
                .gap_8()
                .child(title)
                .child(self.render_permissions_settings(cx)),
            SettingsTab::Privacy => div()
                .flex()
                .flex_col()
                .gap_8()
                .child(title)
                .child(self.render_privacy_settings(cx)),
            SettingsTab::Meetings => div()
                .flex()
                .flex_col()
                .gap_8()
                .child(title)
                .child(self.render_meeting_settings(cx)),
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

    /// A `SettingSwitchRow` bound to a boolean setting; `disabled` renders the
    /// switch at half opacity and ignores clicks, like the Radix switch.
    fn switch_setting_row(&self, row: SwitchRow, cx: &Context<Self>) -> Div {
        let theme = self.theme;
        let SwitchRow {
            id,
            title,
            description,
            key,
            legacy_path,
            default,
            disabled,
        } = row;
        let checked = self
            .provider_settings
            .bool_setting(key, legacy_path, default);
        let switch = render_switch(
            theme,
            id,
            checked,
            cx.listener(move |this, _: &ClickEvent, _, cx| {
                if !disabled {
                    this.set_bool_setting(key, !checked, cx);
                }
            }),
        )
        .when(disabled, |switch| switch.opacity(0.5).cursor_not_allowed());
        setting_row(theme, title, description, false, switch.into_any_element())
    }

    /// `NotificationSettingsView`: the master switch, then every alert switch
    /// disabled while notifications are off. Platform gating follows the web
    /// view: no Dock bounce row on macOS without the Dock icon, and no
    /// microphone detection on Windows.
    fn render_notification_settings(&self, cx: &Context<Self>) -> Div {
        let theme = self.theme;
        let settings = &self.provider_settings;
        let disabled_all = settings.bool_setting(
            "notification_disabled",
            &["notification", "disabled"],
            false,
        );
        let completion_sound = settings.bool_setting(
            "notification_completion_sound",
            &["notification", "completion_sound"],
            true,
        );
        let sound_name = settings
            .string_setting(
                "notification_completion_sound_name",
                &["notification", "completion_sound_name"],
            )
            .unwrap_or_else(|| "ready".to_string());
        let detect =
            settings.bool_setting("notification_detect", &["notification", "detect"], true);
        let threshold = settings
            .value("mic_active_threshold", &["general", "mic_active_threshold"])
            .and_then(|value| value.as_f64())
            .unwrap_or(15.0) as i64;
        let show_bounce = !cfg!(target_os = "macos")
            || settings.bool_setting("show_app_in_dock", &["general", "show_app_in_dock"], true);
        let supports_mic_detection = !cfg!(target_os = "windows");

        let row = |id, title, description, key, legacy_path, default| {
            self.switch_setting_row(
                SwitchRow {
                    id,
                    title,
                    description: Some(description),
                    key,
                    legacy_path,
                    default,
                    disabled: disabled_all,
                },
                cx,
            )
        };

        let mut page = div()
            .flex()
            .flex_col()
            .gap_6()
            .child(self.switch_setting_row(
                SwitchRow {
                    id: "setting-notification-disabled",
                    title: "Disable all notifications",
                    description: Some("Hide all notification panels, Dock alerts, and completion sounds."),
                    key: "notification_disabled",
                    legacy_path: &["notification", "disabled"],
                    default: false,
                    disabled: false,
                },
                cx,
            ))
            .child(row(
                "setting-notification-transcription",
                "Transcription complete",
                "Show when a transcript is ready.",
                "notification_transcription_complete",
                &["notification", "transcription_complete"],
                true,
            ))
            .child(row(
                "setting-notification-summary",
                "Summary complete",
                "Show when a summary is ready.",
                "notification_summary_complete",
                &["notification", "summary_complete"],
                true,
            ))
            .child(row(
                "setting-notification-cloudsync",
                "Cloud sync complete",
                "Show when initial cloud sync finishes.",
                "notification_cloudsync_complete",
                &["notification", "cloudsync_complete"],
                true,
            ))
            .child(row(
                "setting-notification-event",
                "Event notifications",
                "Prepare for events with a 5-minute reminder.",
                "notification_event",
                &["notification", "event"],
                true,
            ))
            .child(row(
                "setting-notification-recording",
                "Recording status prompts",
                "Ask before stopping when a meeting may have ended. When alerts are off, Anarlog keeps listening.",
                "notification_recording",
                &["notification", "recording"],
                true,
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(row(
                        "setting-notification-sound",
                        "Completion sound",
                        "Play a sound when transcription, summaries, or cloud sync finish.",
                        "notification_completion_sound",
                        &["notification", "completion_sound"],
                        true,
                    ))
                    .when(completion_sound, |group| {
                        // `border-muted ml-3 border-l-2 pl-4`: the Sound select
                        // (`min-w-0 flex-1`) beside the `Preview` outline button.
                        let select = self.render_select(
                            SelectSpec::for_setting(
                                "notification_completion_sound_name",
                                Some(sound_name.clone()),
                                "Select sound",
                                [
                                    ("ready", "Ready"),
                                    ("success", "Success"),
                                    ("chime", "Chime"),
                                    ("sparkle", "Sparkle"),
                                    ("bloom", "Bloom"),
                                ]
                                .into_iter()
                                .map(|(value, label)| SelectOption {
                                    value: value.to_string(),
                                    label: label.to_string(),
                                    detail: None,
                                    glyph: None,
                                })
                                .collect(),
                            ),
                            cx,
                        );
                        group.child(
                            div()
                                .ml_3()
                                .pl_4()
                                .border_l_2()
                                .border_color(theme.muted)
                                .child(setting_row(
                                    theme,
                                    "Sound",
                                    Some("Choose from five completion sounds."),
                                    true,
                                    div()
                                        .flex()
                                        .w_full()
                                        .items_center()
                                        .gap_2()
                                        .child(div().min_w_0().flex_1().w(px(120.0)).child(select))
                                        .child(
                                            div()
                                                .id("notification-sound-preview")
                                                .relative()
                                                .flex()
                                                .items_center()
                                                .h(px(32.0))
                                                // `Button variant="outline"`: the control squircle.
                                                .child(crate::squircle::squircle(
                                                    crate::squircle::CONTROL_RADIUS,
                                                    Some(if self.hovered == Some("notification-sound-preview") {
                                                        theme.accent
                                                    } else {
                                                        theme.card
                                                    }),
                                                    Some((1.0, theme.border)),
                                                ))
                                                .px_3()
                                                .tw_text_xs()
                                                .font_weight(gpui::FontWeight::MEDIUM)
                                                .text_color(theme.foreground)
                                                .when(disabled_all, |button| button.opacity(0.5))
                                                .when(!disabled_all, |button| {
                                                    button.cursor_pointer().on_hover(cx.listener(
                                                        |this, hovering: &bool, _, cx| {
                                                            this.set_hovered("notification-sound-preview", *hovering, cx);
                                                        },
                                                    ))
                                                })
                                                .child("Preview"),
                                        )
                                        .into_any_element(),
                                )),
                        )
                    }),
            );
        if show_bounce {
            page = page.child(row(
                "setting-notification-bounce",
                "Bounce app icon",
                "Get your attention when Anarlog finishes work in the background.",
                "notification_bounce",
                &["notification", "bounce"],
                true,
            ));
        }
        if supports_mic_detection {
            page = page.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(row(
                        "setting-notification-detect",
                        "Microphone detection",
                        "Detect meetings from microphone activity.",
                        "notification_detect",
                        &["notification", "detect"],
                        true,
                    ))
                    .when(detect, |group| {
                        let delay = self.render_select(
                            SelectSpec {
                                id: "mic_active_threshold",
                                current: Some(threshold.to_string()),
                                placeholder: "Select delay",
                                options: Rc::new(
                                    [
                                        ("5", "5 sec"),
                                        ("10", "10 sec"),
                                        ("15", "15 sec"),
                                        ("30", "30 sec"),
                                        ("60", "1 min"),
                                        ("120", "2 min"),
                                    ]
                                    .into_iter()
                                    .map(|(value, label)| SelectOption {
                                        value: value.to_string(),
                                        label: label.to_string(),
                                        detail: None,
                                        glyph: None,
                                    })
                                    .collect(),
                                ),
                                search: None,
                                on_select: Rc::new(|this, value, _, cx| {
                                    if let Ok(seconds) = value.parse::<i64>() {
                                        this.set_setting("mic_active_threshold", serde_json::Value::from(seconds), cx);
                                    }
                                }),
                            },
                            cx,
                        );
                        group.child(
                            div()
                                .ml_3()
                                .pl_4()
                                .border_l_2()
                                .border_color(theme.muted)
                                .flex()
                                .flex_col()
                                .gap_4()
                                .child(setting_row(
                                    theme,
                                    "Detection delay",
                                    Some("Wait before treating microphone activity as a meeting."),
                                    false,
                                    div().w(px(100.0)).child(delay).into_any_element(),
                                ))
                                .child(
                                    div()
                                        .child(
                                            div()
                                                .mb_1()
                                                .tw_text_sm()
                                                .font_weight(gpui::FontWeight::MEDIUM)
                                                .text_color(theme.foreground)
                                                .child("Exclude apps from detection"),
                                        )
                                        .child(
                                            div()
                                                .tw_text_xs()
                                                .text_color(theme.muted_foreground)
                                                .child("Prevent selected apps from triggering meeting detection."),
                                        ),
                                ),
                        )
                    }),
            );
        }
        page
    }

    /// `buildWebAppUrl("/auth")` with the desktop flow and deep-link scheme.
    pub(super) fn auth_url(&self) -> String {
        format!(
            "{}/auth?flow=desktop&scheme={}",
            web_app_url(),
            deep_link_scheme(self.store.identifier())
        )
    }

    /// `SettingsAccount` while signed out: the sign-in section with the
    /// `rounded-pill h-10 border-2 px-6` primary button that opens
    /// `buildWebAppUrl("/auth")` in the browser.
    fn render_account_signed_out(&self, cx: &Context<Self>) -> Div {
        let theme = self.theme;
        let hovered = self.hovered == Some("account-get-started");
        let url = self.auth_url();
        div()
            .flex()
            .min_w_0()
            .flex_col()
            .items_start()
            .gap_4()
            .pb_4()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .tw_text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(theme.foreground)
                            .child("Sign in to Anarlog"),
                    )
                    .child(
                        div()
                            .tw_text_sm()
                            .text_color(theme.muted_foreground)
                            .child("Sign in for cloud transcription, AI models, and sharing."),
                    ),
            )
            .child(
                div()
                    .id("account-get-started")
                    .flex()
                    .h(px(40.0))
                    .items_center()
                    .px_6()
                    .rounded_full()
                    .border_2()
                    .border_color(theme.primary)
                    .bg(if hovered {
                        alpha(theme.primary, 0.9)
                    } else {
                        theme.primary
                    })
                    .shadow(vec![gpui::BoxShadow {
                        // `shadow-[0_4px_14px_rgba(87,83,78,0.4)]`
                        color: alpha(gpui::rgb(0x57534e), 0.4).into(),
                        offset: gpui::point(px(0.0), px(4.0)),
                        blur_radius: px(14.0),
                        spread_radius: px(0.0),
                    }])
                    .tw_text_sm()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.primary_foreground)
                    .cursor_pointer()
                    .on_hover(cx.listener(|this, hovering: &bool, _, cx| {
                        this.set_hovered("account-get-started", *hovering, cx);
                    }))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(move |_, _, cx| cx.open_url(&url))
                    .child("Get started"),
            )
    }

    /// `GuestPlanSection` + `PlanTierList` (wide layout: a two-column grid,
    /// `gap-x-10 gap-y-8`) over `PLAN_TIERS`, with Free current.
    fn render_guest_plans(&self) -> Div {
        let theme = self.theme;
        let tiers = plan_tiers();
        let tier = |tier: &PlanTier| {
            let is_pro = tier.id == "pro";
            let is_current = tier.id == "free";
            let mut header = div()
                .mb_2()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .tw_text_base()
                        .font_weight(if is_pro {
                            gpui::FontWeight::SEMIBOLD
                        } else {
                            gpui::FontWeight::MEDIUM
                        })
                        .text_color(theme.foreground)
                        .child(SharedString::from(tier.name)),
                );
            if is_current {
                // `PlanStatusChip`: `rounded-pill px-2 py-0.5 text-[10px] font-medium bg-muted`.
                header = header.child(
                    div()
                        .rounded_full()
                        .px_2()
                        .py(px(2.0))
                        .bg(theme.muted)
                        .text_size(px(10.0))
                        .line_height(px(15.0))
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(theme.muted_foreground)
                        .child("Current"),
                );
            }
            let mut price = div().mb_2().child(
                div()
                    .flex()
                    .items_baseline()
                    .child(
                        div()
                            .tw_text_lg()
                            .text_size(px(20.0))
                            .line_height(px(28.0))
                            .text_color(theme.muted_foreground)
                            .child(SharedString::from(tier.price)),
                    )
                    .when(!tier.period.is_empty(), |row| {
                        row.child(
                            div()
                                .ml_1()
                                .tw_text_sm()
                                .text_color(theme.muted_foreground)
                                .child(SharedString::from(tier.period)),
                        )
                    }),
            );
            if let Some(subtitle) = tier.subtitle {
                price = price.child(
                    div()
                        .mt(px(2.0))
                        .tw_text_xs()
                        .text_color(theme.muted_foreground)
                        .child(SharedString::from(subtitle)),
                );
            }
            let details =
                if tier.id == "free" {
                    div()
                        .tw_text_xs()
                        .text_color(theme.muted_foreground)
                        .child("On-device transcription, recordings, and your own keys.")
                } else {
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(
                            div()
                                .tw_text_xs()
                                .line_height(px(20.0))
                                .text_color(theme.muted_foreground)
                                .child(SharedString::from(tier.description)),
                        )
                        .child(
                            // `PlanFeatureList dense`: `gap-1.5` rows, 14px emerald check.
                            div().flex().flex_col().gap(px(6.0)).children(
                                tier.features.iter().map(|feature| {
                                    div()
                                        .flex()
                                        .items_start()
                                        .gap(px(6.0))
                                        .child(
                                            div()
                                                .flex()
                                                .h(px(16.0))
                                                .flex_shrink_0()
                                                .items_center()
                                                .child(icon(
                                                    "check-circle",
                                                    px(14.0),
                                                    gpui::rgb(0x009966),
                                                )),
                                        )
                                        .child(
                                            div()
                                                .flex_1()
                                                .flex()
                                                .min_h(px(16.0))
                                                .items_center()
                                                .tw_text_xs()
                                                .text_color(theme.foreground)
                                                .child(SharedString::from(*feature)),
                                        )
                                }),
                            ),
                        )
                };
            div()
                .flex()
                .flex_col()
                .child(header)
                .child(price)
                .child(details)
        };
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .mb_4()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .tw_text_lg()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.foreground)
                            .child("Plans"),
                    )
                    .child(
                        div()
                            .tw_text_sm()
                            .text_color(theme.muted_foreground)
                            .child("Compare Free, Pro, Team, and Enterprise."),
                    ),
            )
            .child(
                // `grid grid-cols-2 gap-x-10 gap-y-8`
                div()
                    .flex()
                    .flex_col()
                    .gap_8()
                    .children(tiers.chunks(2).map(|pair| {
                        div()
                            .flex()
                            .gap_10()
                            .children(pair.iter().map(|t| div().flex_1().min_w_0().child(tier(t))))
                    })),
            )
    }

    /// `usePermission(...).check`: probe on a blocking thread and store the
    /// status.
    fn check_permission(&mut self, permission: &'static str, cx: &mut Context<Self>) {
        let audio = cx.global::<crate::audio::Audio>().0.clone();
        let task = self
            .store
            .runtime()
            .spawn_blocking(move || match permission {
                "microphone" => crate::audio::check_microphone(audio.as_ref()),
                _ => crate::audio::check_system_audio(audio.as_ref()),
            });
        cx.spawn(async move |this, cx| {
            if let Ok(status) = task.await {
                this.update(cx, |this, cx| {
                    let state = this.permissions.entry(permission).or_default();
                    state.status = Some(status);
                    state.pending = false;
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    /// `usePermission(...).request`: run the platform request, record its
    /// error, then re-check.
    fn request_permission(&mut self, permission: &'static str, cx: &mut Context<Self>) {
        let audio = cx.global::<crate::audio::Audio>().0.clone();
        let state = self.permissions.entry(permission).or_default();
        state.pending = true;
        state.error = None;
        let task = self
            .store
            .runtime()
            .spawn_blocking(move || match permission {
                "microphone" => crate::audio::request_microphone(audio.as_ref()),
                _ => crate::audio::request_system_audio(audio.as_ref()),
            });
        cx.spawn(async move |this, cx| {
            let result = task.await.unwrap_or_else(|error| Err(error.to_string()));
            this.update(cx, |this, cx| {
                let state = this.permissions.entry(permission).or_default();
                state.error = result.err();
                this.check_permission(permission, cx);
            })
            .ok();
        })
        .detach();
    }

    /// `Permissions` off macOS: the Audio group with runtime capabilities.
    fn render_permissions_settings(&self, cx: &Context<Self>) -> Div {
        let theme = self.theme;
        div()
            .flex()
            .flex_col()
            .child(
                // `PermissionGroup`: `text-xs font-semibold tracking-wide uppercase mb-3`.
                div()
                    .mb_3()
                    .tw_text_xs()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme.muted_foreground)
                    .child("AUDIO"),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(self.render_permission_row(
                        "microphone",
                        "Microphone",
                        "Record your voice in meetings and calls.",
                        cx,
                    ))
                    .child(self.render_permission_row(
                        "system_audio",
                        "System audio",
                        "Record other participants in meetings.",
                        cx,
                    )),
            )
    }

    /// `PermissionRow` with `runtimeCapability`: red title + warning glyph until
    /// authorized, a `size-8` button that requests (arrow) or, once granted,
    /// shows the green check disabled.
    fn render_permission_row(
        &self,
        permission: &'static str,
        title: &'static str,
        description: &'static str,
        cx: &Context<Self>,
    ) -> Div {
        let theme = self.theme;
        let state = self.permissions.get(permission);
        let authorized = state
            .is_some_and(|state| state.status == Some(crate::audio::PermissionStatus::Authorized));
        let pending = state.is_some_and(|state| state.pending);
        let error = state.and_then(|state| state.error.clone());
        let red = gpui::rgb(0xfb2c36);
        let green = gpui::rgb(0x00a63e);
        let title_color = if authorized { theme.foreground } else { red };
        let hover_id: &'static str = match permission {
            "microphone" => "permission-microphone",
            _ => "permission-system-audio",
        };
        let hovered = self.hovered == Some(hover_id);
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap_4()
            .child(
                div()
                    .flex_1()
                    .child(
                        div()
                            .mb_1()
                            .flex()
                            .items_center()
                            .gap_2()
                            .when(!authorized, |row| {
                                row.child(icon("warning-circle", px(16.0), red))
                            })
                            .child(
                                div()
                                    .tw_text_sm()
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .text_color(title_color)
                                    .child(title),
                            ),
                    )
                    .child(
                        div()
                            .tw_text_xs()
                            .text_color(theme.muted_foreground)
                            .child(description),
                    )
                    .when_some(error, |column, error| {
                        column.child(
                            div()
                                .mt_1()
                                .tw_text_xs()
                                .text_color(red)
                                .child(SharedString::from(error)),
                        )
                    }),
            )
            .child(if authorized {
                // `variant="ghost"` disabled: the green check with no hover.
                div()
                    .size(px(32.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .opacity(0.5)
                    .child(icon("check", px(16.0), green))
                    .into_any_element()
            } else {
                // `variant="default" size="icon"`: `bg-primary text-primary-foreground`.
                div()
                    .id(SharedString::from(format!("permission-{permission}")))
                    .relative()
                    .size(px(32.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(crate::squircle::squircle(
                        crate::squircle::CONTROL_RADIUS,
                        Some(if hovered {
                            alpha(theme.primary, 0.9)
                        } else {
                            theme.primary
                        }),
                        None,
                    ))
                    .when(pending, |button| button.opacity(0.5))
                    .when(!pending, |button| {
                        button
                            .cursor_pointer()
                            .on_hover(cx.listener(move |this, hovering: &bool, _, cx| {
                                this.set_hovered(hover_id, *hovering, cx);
                            }))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.request_permission(permission, cx);
                            }))
                    })
                    .child(icon("arrow-right", px(20.0), theme.primary_foreground))
                    .into_any_element()
            })
    }

    /// `SettingsPrivacy`: Lock app (disabled where device authentication is
    /// unavailable, as on Linux), usage data (PostHog), and error reports.
    fn render_privacy_settings(&self, cx: &Context<Self>) -> Div {
        let auth_available = cfg!(any(target_os = "macos", target_os = "windows"));
        let lock_description = if !auth_available {
            "Device authentication is not available on this computer."
        } else if cfg!(target_os = "windows") {
            "Require Windows Hello face, PIN, or password when opening Anarlog."
        } else {
            "Require Touch ID or your password when opening Anarlog."
        };
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(self.switch_setting_row(
                SwitchRow {
                    id: "setting-lock-app",
                    title: "Lock app",
                    description: Some(lock_description),
                    key: "lock_app",
                    legacy_path: &["general", "lock_app"],
                    // `checked={lockAppEnabled && authAvailable}`
                    default: false,
                    disabled: !auth_available,
                },
                cx,
            ))
            .child(self.switch_setting_row(
                SwitchRow {
                    id: "setting-telemetry",
                    title: "Share usage data (PostHog)",
                    description: Some("Help improve Anarlog with anonymous usage data."),
                    key: "telemetry_consent",
                    legacy_path: &["general", "telemetry_consent"],
                    default: true,
                    disabled: false,
                },
                cx,
            ))
            .child(self.switch_setting_row(
                SwitchRow {
                    id: "setting-crash-reports",
                    title: "Error",
                    description: Some(
                        "Send sanitized crash and error reports to help improve Anarlog.",
                    ),
                    key: "crash_reporting_consent",
                    legacy_path: &["general", "crash_reporting_consent"],
                    default: true,
                    disabled: false,
                },
                cx,
            ))
    }

    /// The Meetings page: `DefaultMeetingShareAccessSelector`,
    /// `MeetingSettingsView`, then the Summaries and Audio sections.
    fn render_meeting_settings(&self, cx: &Context<Self>) -> Div {
        let theme = self.theme;
        let settings = &self.provider_settings;
        let auto_start = settings.bool_setting(
            "auto_start_scheduled_meetings",
            &["general", "auto_start_scheduled_meetings"],
            true,
        );
        let share_access = settings
            .string_setting(
                "default_meeting_share_access",
                &["general", "default_meeting_share_access"],
            )
            .unwrap_or_else(|| "me".to_string());
        let summary_length = settings
            .string_setting("summary_length", &["general", "summary_length"])
            .unwrap_or_else(|| "detailed".to_string());
        let retention = settings
            .string_setting("audio_retention", &["general", "audio_retention"])
            .unwrap_or_else(|| "forever".to_string());
        let microphone = settings
            .string_setting("microphone_device", &["general", "microphone_device"])
            .unwrap_or_default();
        let supports_meeting_ax = cfg!(any(target_os = "macos", target_os = "linux"));
        let supports_mic_detection = !cfg!(target_os = "windows");

        let options = |pairs: &[(&str, &str)]| {
            pairs
                .iter()
                .map(|(value, label)| SelectOption {
                    value: value.to_string(),
                    label: label.to_string(),
                    detail: None,
                    glyph: None,
                })
                .collect::<Vec<_>>()
        };

        let mut meetings = div()
            .flex()
            .flex_col()
            .gap_4()
            .child(setting_row(
                theme,
                "Default sharing",
                Some("Choose who can access notes from new meetings."),
                true,
                self.render_select(
                    SelectSpec::for_setting(
                        "default_meeting_share_access",
                        Some(share_access),
                        "Select default sharing",
                        options(&[
                            ("me", "Only me"),
                            ("participants", "People in the meeting"),
                            ("workspace", "Everyone in the workspace"),
                        ]),
                    ),
                    cx,
                ),
            ))
            .child(self.switch_setting_row(
                SwitchRow {
                    id: "setting-auto-start",
                    title: "Start when meeting begins",
                    description: Some("Start listening when a scheduled meeting begins."),
                    key: "auto_start_scheduled_meetings",
                    legacy_path: &["general", "auto_start_scheduled_meetings"],
                    default: true,
                    disabled: false,
                },
                cx,
            ))
            .child(self.switch_setting_row(
                SwitchRow {
                    id: "setting-auto-join",
                    title: "Join scheduled meetings",
                    description: Some("Open the meeting link when a scheduled meeting begins."),
                    key: "auto_join_scheduled_meetings",
                    legacy_path: &["general", "auto_join_scheduled_meetings"],
                    default: false,
                    disabled: !auto_start,
                },
                cx,
            ));
        if supports_mic_detection {
            meetings = meetings.child(self.switch_setting_row(
                SwitchRow {
                    id: "setting-auto-stop",
                    title: "Stop when meeting ends",
                    description: Some("Stop listening when your call ends."),
                    key: "auto_stop_meetings",
                    legacy_path: &["general", "auto_stop_meetings"],
                    default: true,
                    disabled: false,
                },
                cx,
            ));
        }
        if supports_meeting_ax {
            meetings = meetings
                .child(self.switch_setting_row(
                SwitchRow {
                    id: "setting-consent-chat",
                    title: "Post recording disclosure in meeting chat",
                    description: Some("Tell participants when listening starts; this does not confirm consent."),
                    key: "consent_auto_send_chat",
                    legacy_path: &["general", "consent_auto_send_chat"],
                    default: false,
                    disabled: false,
                },
                cx,
            ))
                .child(self.switch_setting_row(
                SwitchRow {
                    id: "setting-capture-chat",
                    title: "Capture meeting chat in Memos",
                    description: Some("Save visible chat from supported meetings using Accessibility."),
                    key: "capture_meeting_chat",
                    legacy_path: &["general", "capture_meeting_chat"],
                    default: false,
                    disabled: false,
                },
                cx,
            ));
        }
        meetings = meetings.child(self.switch_setting_row(
            SwitchRow {
                id: "setting-floating-bar",
                title: "Show floating bar",
                description: Some("Control listening without reopening Anarlog."),
                key: "floating_bar_enabled",
                legacy_path: &["general", "floating_bar_enabled"],
                default: true,
                disabled: false,
            },
            cx,
        ));

        div()
            .flex()
            .flex_col()
            .gap_8()
            .child(meetings)
            .child(
                div().child(section_heading(theme, "Summaries")).child(setting_row(
                    theme,
                    "Summary length",
                    Some("Choose how much detail generated meeting summaries include."),
                    true,
                    self.render_select(
                        SelectSpec::for_setting(
                            "summary_length",
                            Some(summary_length),
                            "Select length",
                            options(&[
                                ("crisp", "Crisp"),
                                ("balanced", "Balanced"),
                                ("detailed", "Detailed"),
                            ]),
                        ),
                        cx,
                    ),
                )),
            )
            .child(
                div().child(section_heading(theme, "Audio")).child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_4()
                        .child(setting_row(
                            theme,
                            "Microphone",
                            Some("Choose the microphone that captures your voice."),
                            true,
                            self.render_select(
                                SelectSpec::for_setting(
                                    "microphone_device",
                                    Some(microphone.clone()),
                                    "Select microphone",
                                    // The device list comes from the audio plugin; only
                                    // the system default is offered until it is ported.
                                    std::iter::once(SelectOption {
                                        value: String::new(),
                                        label: "Current default".to_string(),
                                        detail: None,
                                        glyph: None,
                                    })
                                    .chain((!microphone.is_empty()).then(|| SelectOption {
                                        value: microphone.clone(),
                                        label: format!("{microphone} (Unavailable — using current default)"),
                                        detail: None,
                                        glyph: None,
                                    }))
                                    .collect(),
                                ),
                                cx,
                            ),
                        ))
                        .child(self.switch_setting_row(
                SwitchRow {
                    id: "setting-remember-speakers",
                    title: "Remember speakers",
                    description: Some(
                                "Build voiceprints from meeting audio so speakers you name in a transcript are recognized in later meetings. Voiceprints never leave this device, and unnamed ones are deleted after 45 days.",
                            ),
                    key: "remember_speakers",
                    legacy_path: &["general", "remember_speakers"],
                    default: true,
                    disabled: false,
                },
                cx,
            ))
                        .child(setting_row(
                            theme,
                            "Audio file retention",
                            Some("Choose how long recordings stay on this device."),
                            true,
                            self.render_select(
                                SelectSpec::for_setting(
                                    "audio_retention",
                                    Some(retention),
                                    "Select retention",
                                    options(&[
                                        ("none", "Don't save"),
                                        ("oneDay", "1 day"),
                                        ("threeDays", "3 days"),
                                        ("oneWeek", "1 week"),
                                        ("oneMonth", "1 month"),
                                        ("forever", "Forever"),
                                    ]),
                                ),
                                cx,
                            ),
                        )),
                ),
            )
    }

    /// `AppSettingsView` + Language & Region + Storage.
    fn render_general_settings(&self, window: &Window, cx: &Context<Self>) -> Div {
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
            .string_setting("ai_language", &["language", "ai_language"])
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
                                self.render_select(
                                    SelectSpec {
                                        id: "ai_language",
                                        // `normalizedValue`: `en-US` selects the `en` option.
                                        current: Some(base_language_code(&language)),
                                        placeholder: "Select language",
                                        options: Rc::new(
                                            CORE_LANGUAGES
                                                .iter()
                                                .map(|(code, label)| SelectOption {
                                                    value: code.to_string(),
                                                    label: label.to_string(),
                                                    detail: None,
                                                    glyph: None,
                                                })
                                                .collect(),
                                        ),
                                        search: Some(SearchSpec {
                                            placeholder: "Search language...",
                                            empty_message: "No matching languages found",
                                            width: None,
                                        }),
                                        on_select: Rc::new(|this, value, _, cx| {
                                            // Changing the main language also drops it from
                                            // `spoken_languages` (`getAdditionalSpokenLanguages`).
                                            let spoken = this
                                                .provider_settings
                                                .string_setting(
                                                    "spoken_languages",
                                                    &["language", "spoken_languages"],
                                                )
                                                .and_then(|json| {
                                                    serde_json::from_str::<Vec<String>>(&json).ok()
                                                })
                                                .unwrap_or_default();
                                            let spoken =
                                                additional_spoken_languages(&value, &spoken);
                                            this.set_setting(
                                                "ai_language",
                                                serde_json::Value::String(value),
                                                cx,
                                            );
                                            this.set_setting(
                                                "spoken_languages",
                                                serde_json::Value::String(
                                                    serde_json::to_string(&spoken)
                                                        .unwrap_or_else(|_| "[]".to_string()),
                                                ),
                                                cx,
                                            );
                                        }),
                                    },
                                    cx,
                                ),
                            ))
                            .child(setting_row(
                                theme,
                                "Timezone",
                                Some("Show the timeline in your preferred timezone."),
                                true,
                                self.render_select(
                                    SelectSpec {
                                        id: "timezone",
                                        // `displayValue = value || systemTimezone`
                                        current: Some(
                                            timezone.clone().unwrap_or_else(system_timezone),
                                        ),
                                        placeholder: "Select timezone",
                                        options: Rc::new(
                                            COMMON_TIMEZONES
                                                .iter()
                                                .map(|(value, label, detail)| SelectOption {
                                                    value: value.to_string(),
                                                    label: label.to_string(),
                                                    detail: Some(detail),
                                                    glyph: None,
                                                })
                                                .collect(),
                                        ),
                                        search: Some(SearchSpec {
                                            placeholder: "Search timezone...",
                                            empty_message: "No results found.",
                                            width: Some(288.0),
                                        }),
                                        on_select: Rc::new(|this, value, _, cx| {
                                            // Picking the system zone stores "" (`handleChange`).
                                            let stored = if value == system_timezone() {
                                                String::new()
                                            } else {
                                                value
                                            };
                                            this.set_setting(
                                                "timezone",
                                                serde_json::Value::String(stored),
                                                cx,
                                            );
                                        }),
                                    },
                                    cx,
                                ),
                            ))
                            .child(setting_row(
                                theme,
                                "Week starts on",
                                Some("Choose which day begins your calendar week."),
                                true,
                                self.render_select(
                                    SelectSpec::for_setting(
                                        "week_start",
                                        Some(week_start.clone()),
                                        "Select day",
                                        vec![
                                            SelectOption {
                                                value: "sunday".to_string(),
                                                label: "Sunday".to_string(),
                                                detail: None,
                                                glyph: None,
                                            },
                                            SelectOption {
                                                value: "monday".to_string(),
                                                label: "Monday".to_string(),
                                                detail: None,
                                                glyph: None,
                                            },
                                        ],
                                    ),
                                    cx,
                                ),
                            ))
                            .child(self.render_spoken_languages(window, cx)),
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
                                .relative()
                                .w(px(144.0))
                                .h(px(36.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(crate::squircle::squircle(
                                    crate::squircle::CONTROL_RADIUS,
                                    Some(theme.background),
                                    Some((1.0, theme.border)),
                                ))
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
                                    .rounded(px(8.0))
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

/// `CORE_TRANSCRIPTION_LANGUAGE_CODES` with their English
/// `Intl.DisplayNames` labels, in the app's order.
const CORE_LANGUAGES: [(&str, &str); 49] = [
    ("ar", "Arabic"),
    ("be", "Belarusian"),
    ("bg", "Bulgarian"),
    ("bn", "Bangla"),
    ("bs", "Bosnian"),
    ("ca", "Catalan"),
    ("cs", "Czech"),
    ("da", "Danish"),
    ("de", "German"),
    ("el", "Greek"),
    ("en", "English"),
    ("es", "Spanish"),
    ("et", "Estonian"),
    ("fa", "Persian"),
    ("fi", "Finnish"),
    ("fr", "French"),
    ("he", "Hebrew"),
    ("hi", "Hindi"),
    ("hr", "Croatian"),
    ("hu", "Hungarian"),
    ("id", "Indonesian"),
    ("it", "Italian"),
    ("ja", "Japanese"),
    ("kn", "Kannada"),
    ("ko", "Korean"),
    ("lt", "Lithuanian"),
    ("lv", "Latvian"),
    ("mk", "Macedonian"),
    ("mr", "Marathi"),
    ("ms", "Malay"),
    ("nl", "Dutch"),
    ("no", "Norwegian"),
    ("pl", "Polish"),
    ("pt", "Portuguese"),
    ("ro", "Romanian"),
    ("ru", "Russian"),
    ("sk", "Slovak"),
    ("sl", "Slovenian"),
    ("sr", "Serbian"),
    ("sv", "Swedish"),
    ("ta", "Tamil"),
    ("te", "Telugu"),
    ("th", "Thai"),
    ("tl", "Filipino"),
    ("tr", "Turkish"),
    ("uk", "Ukrainian"),
    ("ur", "Urdu"),
    ("vi", "Vietnamese"),
    ("zh", "Chinese"),
];

/// `COMMON_TIMEZONES` from `timezone.tsx`.
const COMMON_TIMEZONES: [(&str, &str, &str); 22] = [
    ("Pacific/Honolulu", "Hawaii", "UTC-10"),
    ("America/Anchorage", "Alaska", "UTC-9"),
    ("America/Los_Angeles", "Pacific Time", "UTC-8"),
    ("America/Denver", "Mountain Time", "UTC-7"),
    ("America/Chicago", "Central Time", "UTC-6"),
    ("America/New_York", "Eastern Time", "UTC-5"),
    ("America/Sao_Paulo", "Sao Paulo", "UTC-3"),
    ("Atlantic/Reykjavik", "Reykjavik", "UTC+0"),
    ("Europe/London", "London", "UTC+0/+1"),
    ("Europe/Paris", "Paris", "UTC+1/+2"),
    ("Europe/Berlin", "Berlin", "UTC+1/+2"),
    ("Africa/Cairo", "Cairo", "UTC+2"),
    ("Europe/Moscow", "Moscow", "UTC+3"),
    ("Asia/Dubai", "Dubai", "UTC+4"),
    ("Asia/Kolkata", "India", "UTC+5:30"),
    ("Asia/Bangkok", "Bangkok", "UTC+7"),
    ("Asia/Singapore", "Singapore", "UTC+8"),
    ("Asia/Shanghai", "China", "UTC+8"),
    ("Asia/Tokyo", "Tokyo", "UTC+9"),
    ("Asia/Seoul", "Seoul", "UTC+9"),
    ("Australia/Sydney", "Sydney", "UTC+10/+11"),
    ("Pacific/Auckland", "Auckland", "UTC+12/+13"),
];

/// `Intl.DateTimeFormat().resolvedOptions().timeZone`
fn system_timezone() -> String {
    iana_time_zone::get_timezone().unwrap_or_else(|_| "UTC".to_string())
}

/// `getBaseLanguageCode`: the primary subtag of a BCP 47 tag.
pub(super) fn base_language_code(code: &str) -> String {
    code.split(['-', '_']).next().unwrap_or(code).to_lowercase()
}

/// `getAdditionalSpokenLanguages`: the stored list minus the main language,
/// de-duplicated by base code.
fn additional_spoken_languages(main: &str, spoken: &[String]) -> Vec<String> {
    let main = base_language_code(main);
    let mut seen = std::collections::HashSet::new();
    spoken
        .iter()
        .map(|code| base_language_code(code))
        .filter(|code| !code.is_empty() && *code != main && seen.insert(code.clone()))
        .collect()
}

/// One `SettingSwitchRow` bound to a boolean setting.
struct SwitchRow {
    id: &'static str,
    title: &'static str,
    description: Option<&'static str>,
    key: &'static str,
    legacy_path: &'static [&'static str],
    default: bool,
    disabled: bool,
}

pub(crate) struct SelectOption {
    pub value: String,
    pub label: String,
    /// `SearchableSelectOption.detail`, shown as `label (detail)` on the
    /// trigger and in `font-mono text-[10px]` on the row.
    pub detail: Option<&'static str>,
    /// `ProviderIconSlot` before the label (the AI provider selects).
    pub glyph: Option<crate::ai_providers::Icon>,
}

/// `PlanTierData` from `packages/pricing/src/tiers.ts` (`PLAN_TIERS`: prices
/// rendered from `MARKETING_PLAN_TIERS`, only the included features).
struct PlanTier {
    id: &'static str,
    name: &'static str,
    price: &'static str,
    period: &'static str,
    subtitle: Option<&'static str>,
    description: &'static str,
    features: &'static [&'static str],
}

fn plan_tiers() -> [PlanTier; 4] {
    [
        PlanTier {
            id: "free",
            name: "Free",
            price: "$0",
            period: "/month",
            subtitle: None,
            description: "Private, local meeting notes with on-device models or your own API keys.",
            features: &[
                "Unlimited on-device transcription",
                "Local recordings and audio player",
                "Bring your own keys for STT and AI",
                "Notes, folders, and templates",
                "Chat and exports",
                "Local API, CLI, MCP, and webhooks",
                "Manual Speaker Labeling",
            ],
        },
        PlanTier {
            id: "pro",
            name: "Pro",
            price: "$15",
            period: "/month",
            subtitle: Some("or $150/year"),
            description: "Hosted transcription, AI, sync, and personal workflows for one person.",
            features: &[
                "Everything in Free",
                "Cloud Transcription",
                "Cloud LLM",
                "Better Speaker Identification",
                "End-to-end encrypted Cloud Sync",
                "Share individual notes",
                "Integrations and personal automations",
                "Folder sharing with access controls",
                "Custom dictionaries and summary formats",
            ],
        },
        PlanTier {
            id: "team",
            name: "Team",
            price: "$20",
            period: "/person/month",
            subtitle: Some("or $200/person/year"),
            description: "A paid shared workspace with Pro for every member; each workspace has its own per-seat billing.",
            features: &[
                "Everything in Pro for every member",
                "Shared workspaces and notes",
                "Members, roles, and invitations",
                "Centralized per-seat billing",
                "Shared team folders",
                "Shared team templates",
                "Shared team automations",
            ],
        },
        PlanTier {
            id: "enterprise",
            name: "Enterprise",
            price: "Custom",
            period: "",
            subtitle: Some("Founder-led rollout"),
            description: "Organization-wide security, policy, and deployment controls with a founder-led rollout.",
            features: &[
                "Everything in Team",
                "Domain SSO and SCIM",
                "Sharing, retention, and consent policies",
                "Usage and audit visibility",
                "Custom workspace subdomain",
                "Customer-hosted capture and data plane",
                "Founder-led security review and rollout",
            ],
        },
    ]
}

/// `env.VITE_APP_URL`: the dev default, or the CD build's value.
fn web_app_url() -> &'static str {
    if cfg!(debug_assertions) {
        "http://localhost:3000"
    } else {
        "https://anarlog.so"
    }
}

/// `getScheme()` in `shared/utils.ts`.
fn deep_link_scheme(identifier: &str) -> &'static str {
    match identifier {
        "com.hyprnote.stable" | "com.hyprnote.Hyprnote" => "anarlog",
        "com.hyprnote.staging" => "anarlog-staging",
        _ => "anarlog-dev",
    }
}

/// `usePermission`'s slice the page renders.
#[derive(Debug, Default, Clone)]
pub(crate) struct PermissionState {
    pub status: Option<crate::audio::PermissionStatus>,
    pub pending: bool,
    pub error: Option<String>,
}

/// `SearchableSelect`'s popover: a `CommandInput` above the filtered list.
pub(crate) struct SearchSpec {
    pub placeholder: &'static str,
    pub empty_message: &'static str,
    /// `dropdownClassName="w-72"`; otherwise the trigger width.
    pub width: Option<f32>,
}

type OnSelect = Rc<dyn Fn(&mut Workspace, String, &mut Window, &mut Context<Workspace>)>;

pub(crate) struct SelectSpec {
    pub id: &'static str,
    pub current: Option<String>,
    pub placeholder: &'static str,
    pub options: Rc<Vec<SelectOption>>,
    pub search: Option<SearchSpec>,
    pub on_select: OnSelect,
}

impl SelectSpec {
    /// A select that writes its value straight to `key`.
    fn for_setting(
        key: &'static str,
        current: Option<String>,
        placeholder: &'static str,
        options: Vec<SelectOption>,
    ) -> Self {
        Self {
            id: key,
            current,
            placeholder,
            options: Rc::new(options),
            search: None,
            on_select: Rc::new(move |this, value, _, cx| {
                this.set_setting(key, serde_json::Value::String(value), cx);
            }),
        }
    }
}

/// The open select popover: which one, its options for keyboard handling,
/// the cmdk-style highlighted row, and the `SearchableSelect` query input.
pub(crate) struct OpenSelect {
    id: &'static str,
    options: Rc<Vec<SelectOption>>,
    on_select: OnSelect,
    highlighted: usize,
    search: Option<gpui::Entity<TextInput>>,
}

/// cmdk's `filter`: case-insensitive substring match on `label detail`.
fn filter_options<'a>(options: &'a [SelectOption], query: &str) -> Vec<&'a SelectOption> {
    let query = query.to_lowercase();
    options
        .iter()
        .filter(|option| {
            let haystack = match option.detail {
                Some(detail) => format!("{} {detail}", option.label),
                None => option.label.clone(),
            };
            haystack.to_lowercase().contains(&query)
        })
        .collect()
}

impl Workspace {
    fn close_select(&mut self, cx: &mut Context<Self>) {
        self.open_select = None;
        cx.notify();
    }

    fn open_select(&mut self, spec: &SelectSpec, window: &mut Window, cx: &mut Context<Self>) {
        let search = spec.search.as_ref().map(|search| {
            let theme = self.theme;
            let input = cx.new(|cx| {
                TextInput::new(
                    search.placeholder,
                    TextInputStyle {
                        text: theme.foreground,
                        placeholder: theme.muted_foreground,
                        selection: theme.selection,
                        underline_when_focused: false,
                        masked: false,
                    },
                    window,
                    cx,
                )
            });
            cx.subscribe_in(
                &input,
                window,
                |this, input, event: &TextInputEvent, window, cx| {
                    let Some(open) = this.open_select.as_mut() else {
                        return;
                    };
                    match event {
                        TextInputEvent::Changed => {
                            open.highlighted = 0;
                            cx.notify();
                        }
                        TextInputEvent::Navigate(delta) => {
                            let count = filter_options(&open.options, input.read(cx).text()).len();
                            if count > 0 {
                                open.highlighted = (open.highlighted as i32 + delta)
                                    .clamp(0, count as i32 - 1)
                                    as usize;
                                cx.notify();
                            }
                        }
                        TextInputEvent::Enter => {
                            let options = open.options.clone();
                            let on_select = open.on_select.clone();
                            let chosen = filter_options(&options, input.read(cx).text())
                                .get(open.highlighted)
                                .map(|option| option.value.clone());
                            this.close_select(cx);
                            this.focus_handle.focus(window);
                            if let Some(value) = chosen {
                                on_select(this, value, window, cx);
                            }
                        }
                        TextInputEvent::Escape => {
                            this.close_select(cx);
                            this.focus_handle.focus(window);
                        }
                        TextInputEvent::Committed | TextInputEvent::BackspaceEmpty => {}
                    }
                },
            )
            .detach();
            input.read(cx).focus_handle(cx).focus(window);
            input
        });
        self.open_select = Some(OpenSelect {
            id: spec.id,
            options: spec.options.clone(),
            on_select: spec.on_select.clone(),
            highlighted: 0,
            search,
        });
        cx.notify();
    }

    pub(super) fn render_select(&self, spec: SelectSpec, cx: &Context<Self>) -> AnyElement {
        let theme = self.theme;
        let selected = spec
            .current
            .as_ref()
            .and_then(|value| spec.options.iter().find(|option| &option.value == value));
        let (text, color) = match selected {
            Some(option) => (
                SharedString::from(match option.detail {
                    Some(detail) => format!("{} ({detail})", option.label),
                    None => option.label.clone(),
                }),
                theme.foreground,
            ),
            None => (SharedString::from(spec.placeholder), theme.muted_foreground),
        };
        let selected_glyph = selected.and_then(|option| option.glyph);
        let id = spec.id;
        let open = self.open_select.as_ref().filter(|open| open.id == id);
        let spec = Rc::new(spec);
        let spec_for_click = spec.clone();

        div()
            .id(SharedString::from(format!("select-{id}")))
            .relative()
            .w_full()
            .child(
                // `SelectTrigger` / the combobox `Button variant="outline"` with
                // `SETTING_CONTROL_CLASS`: `h-9 w-full rounded-full border
                // bg-card px-3 text-sm`, half-opacity caret.
                div()
                    .id(SharedString::from(format!("select-trigger-{id}")))
                    .relative()
                    .flex()
                    .h(px(36.0))
                    .w_full()
                    .items_center()
                    .justify_between()
                    // `useSquircleRef` replaces the pill radius with the
                    // control squircle.
                    .child(crate::squircle::squircle(
                        crate::squircle::CONTROL_RADIUS,
                        Some(theme.card),
                        Some((1.0, theme.border)),
                    ))
                    .px_3()
                    .py_2()
                    .tw_text_sm()
                    .text_color(color)
                    .cursor_default()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        if this.open_select.as_ref().is_some_and(|open| open.id == id) {
                            this.close_select(cx);
                        } else {
                            this.open_select(&spec_for_click, window, cx);
                        }
                    }))
                    .child(
                        div()
                            .min_w_0()
                            .flex()
                            .items_center()
                            .gap_2()
                            .children(selected_glyph.map(|glyph| {
                                super::ai_settings::provider_icon(glyph, px(20.0), theme)
                            }))
                            .child(div().min_w_0().truncate().child(text)),
                    )
                    .child(icon("caret-down", px(16.0), alpha(theme.foreground, 0.5))),
            )
            .when_some(open, |wrapper, open| {
                let panel = match &spec.search {
                    Some(search) => self.render_searchable_panel(&spec, search, open, cx),
                    None => self.render_select_panel(&spec, cx),
                };
                wrapper.child(gpui::deferred(panel).with_priority(1))
            })
            .into_any_element()
    }

    /// `SelectContent position="popper"`: `bg-popover rounded-[18px] border
    /// shadow-md p-1` at the trigger's width, 4px below; items `py-1.5 pr-8
    /// pl-2 text-sm` with the check at `right-2`.
    fn render_select_panel(&self, spec: &SelectSpec, cx: &Context<Self>) -> AnyElement {
        let theme = self.theme;
        let id = spec.id;
        div()
            .id(SharedString::from(format!("select-content-{id}")))
            .occlude()
            .absolute()
            .top(px(40.0))
            .left_0()
            .w_full()
            .min_w(px(128.0))
            .flex()
            .flex_col()
            .p_1()
            .rounded(px(18.0))
            .border_1()
            .border_color(theme.border)
            .bg(theme.popover)
            .shadow_md()
            .on_mouse_down_out(
                cx.listener(|this, _: &gpui::MouseDownEvent, _, cx| this.close_select(cx)),
            )
            .children(spec.options.iter().enumerate().map(|(index, option)| {
                let selected = spec.current.as_deref() == Some(option.value.as_str());
                let value = option.value.clone();
                let on_select = spec.on_select.clone();
                div()
                    .id(SharedString::from(format!("select-option-{id}-{index}")))
                    .relative()
                    .flex()
                    .w_full()
                    .items_center()
                    .py(px(6.0))
                    .pr_8()
                    .pl_2()
                    .rounded(px(14.0))
                    .tw_text_sm()
                    .text_color(theme.foreground)
                    .cursor_default()
                    .hover(move |style| style.bg(theme.accent))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.close_select(cx);
                        on_select(this, value.clone(), window, cx);
                    }))
                    .when_some(option.glyph, |row, glyph| {
                        row.gap_2()
                            .child(super::ai_settings::provider_icon(glyph, px(20.0), theme))
                    })
                    .child(SharedString::from(option.label.clone()))
                    .when(selected, |item| {
                        item.child(div().absolute().right_2().flex().items_center().child(icon(
                            "check",
                            px(16.0),
                            theme.foreground,
                        )))
                    })
            }))
            .into_any_element()
    }

    /// `PopoverContent variant="app"` + `AppFloatingPanel` + `Command`: the
    /// search row (`border-b px-3`, 16px glass at half opacity, `h-10 text-sm`
    /// input) above a `p-1` list capped at 250px; rows `px-2 py-1.5 text-sm
    /// gap-2` with the label, the mono detail, and a check when selected.
    fn render_searchable_panel(
        &self,
        spec: &SelectSpec,
        search: &SearchSpec,
        open: &OpenSelect,
        cx: &Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        let id = spec.id;
        let query = open
            .search
            .as_ref()
            .map(|input| input.read(cx).text().to_string())
            .unwrap_or_default();
        let matches = filter_options(&spec.options, &query);
        let highlighted = open.highlighted;

        let mut panel = div()
            .id(SharedString::from(format!("select-content-{id}")))
            .occlude()
            .absolute()
            .top(px(40.0))
            .left_0()
            .flex()
            .flex_col()
            .p(px(2.0))
            .rounded(px(22.0))
            .border_1()
            .border_color(theme.floating_border)
            .bg(theme.floating_chrome)
            .shadow_lg()
            .on_mouse_down_out(
                cx.listener(|this, _: &gpui::MouseDownEvent, _, cx| this.close_select(cx)),
            );
        panel = match search.width {
            Some(width) => panel.w(px(width)),
            None => panel.w_full(),
        };

        let mut list = div()
            .id(SharedString::from(format!("select-list-{id}")))
            .flex()
            .flex_col()
            .max_h(px(250.0))
            .overflow_y_scroll()
            .p_1();
        if matches.is_empty() {
            list = list.child(
                div()
                    .px_2()
                    .py(px(6.0))
                    .tw_text_sm()
                    .text_color(theme.muted_foreground)
                    .child(search.empty_message),
            );
        } else {
            list = list.children(matches.into_iter().enumerate().map(|(index, option)| {
                let selected = spec.current.as_deref() == Some(option.value.as_str());
                let value = option.value.clone();
                let on_select = spec.on_select.clone();
                div()
                    .id(SharedString::from(format!("select-option-{id}-{index}")))
                    .flex()
                    .w_full()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .py(px(6.0))
                    .rounded(px(14.0))
                    .tw_text_sm()
                    .text_color(theme.foreground)
                    .cursor_pointer()
                    // cmdk `data-[selected=true]:bg-accent`; the pointer moves it.
                    .when(index == highlighted, |row| row.bg(theme.accent))
                    .hover(move |style| style.bg(theme.accent))
                    .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                        if let Some(open) = this.open_select.as_mut()
                            && *hovered
                            && open.highlighted != index
                        {
                            open.highlighted = index;
                            cx.notify();
                        }
                    }))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.close_select(cx);
                        this.focus_handle.focus(window);
                        on_select(this, value.clone(), window, cx);
                    }))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(SharedString::from(option.label.clone())),
                    )
                    .when_some(option.detail, |row, detail| {
                        row.child(
                            div()
                                .flex_shrink_0()
                                .when_some(self.mono_font_family.clone(), |detail, family| {
                                    detail.font_family(family)
                                })
                                .text_size(px(10.0))
                                .text_color(theme.muted_foreground)
                                .child(detail),
                        )
                    })
                    .when(selected, |row| {
                        row.child(icon("check", px(16.0), theme.foreground))
                    })
            }));
        }

        panel
            .child(
                div()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .rounded(px(20.0))
                    .border_1()
                    .border_color(theme.floating_border)
                    .bg(theme.floating_panel)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .h(px(40.0))
                            .px_3()
                            .border_b_1()
                            .border_color(theme.border)
                            .tw_text_sm()
                            .child(icon("search", px(16.0), alpha(theme.foreground, 0.5)))
                            .child(div().w(px(8.0)))
                            .when_some(open.search.clone(), |row, input| {
                                row.child(div().flex_1().min_w_0().child(input))
                            }),
                    )
                    .child(list),
            )
            .into_any_element()
    }
}
