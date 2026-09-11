//! The settings tab: `apps/desktop/src/sidebar/settings.tsx` (the nav that
//! replaces the timeline) and `apps/desktop/src/settings/index.tsx` (the page
//! frame), with the General page from `settings/general`.

use std::rc::Rc;

use gpui::{
    AnyElement, ClickEvent, Context, Div, Focusable as _, MouseButton, SharedString, Stateful,
    Window, div, prelude::*, px, relative, rgb,
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
    Insights,
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
            Self::Insights => "Your insights",
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
                    tab: SettingsTab::Insights,
                    label: "Insights",
                    icon: "chart-line-up",
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
                    icon: "waveform",
                    requires_pro: false,
                },
                NavItem::Tab {
                    tab: SettingsTab::Intelligence,
                    label: "Intelligence",
                    icon: "brain",
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
        // `openNew({ type: "settings" })` replaces the other overlay tabs, and
        // unmounting the sidebar triggers dismisses their dropdowns.
        self.close_open_menus(cx);
        self.close_folders(cx);
        self.close_automations(cx);
        self.close_templates(cx);
        self.close_calendar(cx);
        self.close_contacts(cx);
        self.close_edit_review(cx);
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
                    TextInputEvent::ShiftEnter | TextInputEvent::ModEnter => {}
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
            SettingsTab::Stats | SettingsTab::Insights => self.ensure_stats(cx),
            SettingsTab::Notifications => self.ensure_installed_apps(cx),
            _ => {}
        }
        // `pendingProvider` is the page's own state.
        if tab != SettingsTab::Transcription {
            self.pending_stt_provider = None;
        }
        self.settings_tab = Some(tab);
        if tab == SettingsTab::Intelligence {
            // `useModelMetadata` / `useQuery(["models", ...])` on mount, and the
            // `staleTime: 0` health probe refetching with it.
            self.ensure_llm_models(false, cx);
            self.ensure_llm_health(true, cx);
        }
        if tab == SettingsTab::Transcription {
            self.ensure_deepgram_health(cx);
        }
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
                    // `border border-transparent px-2 py-0.5` as padding.
                    .px(px(9.0))
                    .py(px(3.0))
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

    pub(crate) fn close_settings(&mut self, cx: &mut Context<Self>) {
        if self.settings_tab.take().is_some() {
            cx.notify();
        }
    }

    /// An overlay tab (`RETURN_ORIGIN_TAB_TYPES`) is the active surface.
    pub(crate) fn overlay_tab_open(&self) -> bool {
        self.settings_open()
            || self.folders_open()
            || self.templates_open()
            || self.calendar_open()
            || self.contacts_open()
            || self.automations_open()
    }

    /// `openNew` of an overlay tab records the active tab as its return
    /// origin: the settings tab when the settings nav opened it, else the
    /// note underneath.
    pub(crate) fn remember_overlay_origin(&mut self) {
        self.overlay_return_settings = self.settings_tab;
    }

    /// `leaveOverlayTab`: Escape and the sidebar's back arrow return to the
    /// tab the overlay was opened from — the settings tab for a tab opened
    /// from the settings nav, otherwise the note underneath.
    pub(crate) fn leave_overlay_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings_open() {
            self.close_settings(cx);
            return;
        }
        let return_to = self.overlay_return_settings.take();
        self.close_folders(cx);
        self.close_templates(cx);
        self.close_calendar(cx);
        self.close_contacts(cx);
        self.close_automations(cx);
        if let Some(tab) = return_to {
            self.open_settings(tab, window, cx);
        }
        cx.notify();
    }

    pub(crate) fn settings_open(&self) -> bool {
        self.settings_tab.is_some()
    }

    pub(super) fn set_bool_setting(
        &mut self,
        key: &'static str,
        value: bool,
        cx: &mut Context<Self>,
    ) {
        if key == "crash_reporting_consent" {
            anlg_crash_reporting::set_enabled(value);
        }
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
        if key == "show_tray_icon" {
            // `setSettingValues` → `setTrayIconVisible`: the switch drives the tray at once.
            self.sync_tray(cx);
        }
        if key.starts_with("current_llm_") && !self.applying_llm_default {
            self.ensure_llm_models(false, cx);
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
                self.tooltip_trigger(
                    super::tooltip::TooltipSpec::title("settings-back", "Back"),
                    self.tracked_chrome_button("settings-back", cx)
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                            this.leave_overlay_tab(window, cx)
                        }))
                        .child(icon(
                            "arrow-left",
                            px(16.0),
                            self.chrome_icon_color("settings-back"),
                        )),
                    cx,
                ),
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
                            } else if label == "Automations" {
                                this.open_automations(window, cx);
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
                .child(self.render_notification_settings(window, cx)),
            SettingsTab::Transcription => {
                self.render_ai_settings(super::ai_settings::ProviderKind::Stt, title, window, cx)
            }
            SettingsTab::Intelligence => {
                self.render_ai_settings(super::ai_settings::ProviderKind::Llm, title, window, cx)
            }
            SettingsTab::Developers => self.render_developers_settings(title, window, cx),
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
                                .on_click(cx.listener(|_, _: &gpui::ClickEvent, _, _cx| {
                                    crate::opener::open_url("https://docs.anarlog.so/imports");
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
            SettingsTab::Insights => self.render_insights_settings(title, window, cx),
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
    fn render_notification_settings(&self, window: &Window, cx: &Context<Self>) -> Div {
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
                                    badges: Vec::new(),
                                    lock: None,
                                    heading: None,
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
                                                    let name = sound_name.clone();
                                                    button
                                                        .cursor_pointer()
                                                        .on_hover(cx.listener(
                                                            |this, hovering: &bool, _, cx| {
                                                                this.set_hovered("notification-sound-preview", *hovering, cx);
                                                            },
                                                        ))
                                                        // `previewCompletionSound(normalizeCompletionSoundName(name))`
                                                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                                        .on_click(cx.listener(move |this, _: &gpui::ClickEvent, _, cx| {
                                                            this.preview_completion_sound(
                                                                crate::cuelume::normalize_completion_sound_name(Some(&name)),
                                                                cx,
                                                            );
                                                        }))
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
                                        badges: Vec::new(),
                                        lock: None,
                                        heading: None,
                                    })
                                    .collect(),
                                ),
                                search: None,
                                on_select: Rc::new(|this, value, _, cx| {
                                    if let Ok(seconds) = value.parse::<i64>() {
                                        this.set_setting("mic_active_threshold", serde_json::Value::from(seconds), cx);
                                    }
                                }),
                                combobox: None,
                                align_end: false,
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
                                                .mb_3()
                                                .tw_text_xs()
                                                .text_color(theme.muted_foreground)
                                                .child("Prevent selected apps from triggering meeting detection."),
                                        )
                                        .child(self.render_excluded_apps(window, cx)),
                                ),
                        )
                    }),
            );
        }
        page
    }

    /// `ignored_platforms` / `included_platforms`: JSON arrays stored as
    /// strings, `[]` by default.
    fn platform_setting(&self, key: &str) -> Vec<String> {
        crate::notification_apps::parse_platforms(
            self.provider_settings
                .string_setting(key, &["notification", key])
                .as_deref(),
        )
    }

    /// `handleToggleIgnoredApp`: rewrite both lists and close the picker.
    fn toggle_ignored_app(&mut self, bundle_id: &str, cx: &mut Context<Self>) {
        if bundle_id.is_empty() {
            return;
        }
        let (ignored, included) = crate::notification_apps::toggle_ignored_app(
            bundle_id,
            &self.platform_setting("ignored_platforms"),
            &self.platform_setting("included_platforms"),
            &self.default_ignored_apps,
        );
        let encode = |list: Vec<String>| {
            serde_json::Value::String(serde_json::to_string(&list).unwrap_or_else(|_| "[]".into()))
        };
        self.set_setting("ignored_platforms", encode(ignored), cx);
        self.set_setting("included_platforms", encode(included), cx);
        self.close_select(cx);
    }

    /// `listInstalledApplications` once the page opens (a `useQuery`).
    pub(super) fn ensure_installed_apps(&mut self, cx: &mut Context<Self>) {
        if self.installed_apps.is_some() {
            return;
        }
        self.installed_apps = Some(Rc::new(Vec::new()));
        let task = self
            .store
            .runtime()
            .spawn_blocking(anlg_detect::list_installed_apps);
        cx.spawn(async move |this, cx| {
            let Ok(apps) = task.await else {
                return;
            };
            this.update(cx, |this, cx| {
                this.installed_apps = Some(Rc::new(apps));
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// The excluded-apps `Popover`: a `min-h-[38px] rounded-2xl border p-2`
    /// chip box (the ignored apps as `Badge`s — `bg-accent text-muted-foreground`
    /// with `(default)` for the default ignores, `bg-muted` otherwise — and the
    /// `Search installed apps...` prompt) opening the cmdk panel of the apps
    /// still excludable.
    fn render_excluded_apps(&self, window: &Window, cx: &Context<Self>) -> Div {
        let theme = self.theme;
        let installed = self
            .installed_apps
            .clone()
            .unwrap_or_else(|| Rc::new(Vec::new()));
        let ignored_platforms = self.platform_setting("ignored_platforms");
        let included_platforms = self.platform_setting("included_platforms");
        let defaults = &self.default_ignored_apps;
        let ignored_ids = crate::notification_apps::ignored_bundle_ids(
            &installed,
            &ignored_platforms,
            &included_platforms,
            defaults,
        );
        let options: Vec<SelectOption> = crate::notification_apps::ignorable_apps(
            &installed,
            &ignored_platforms,
            &included_platforms,
            "",
            defaults,
        )
        .into_iter()
        .map(|app| SelectOption {
            value: app.id.clone(),
            label: app.name.clone(),
            detail: None,
            glyph: None,
            badges: Vec::new(),
            lock: None,
            heading: None,
        })
        .collect();
        // Radix `avoidCollisions`: the popover flips above the trigger when
        // its input, padding and up to 250px of rows would not fit below it.
        let panel_height = 46.0 + (8.0 + options.len() as f32 * 32.0).min(250.0);
        let placement = match self.excluded_apps_bounds.get() {
            Some(bounds)
                if f32::from(bounds.bottom()) + 4.0 + panel_height
                    > f32::from(window.viewport_size().height) - 8.0 =>
            {
                PanelPlacement::Above
            }
            _ => PanelPlacement::Below,
        };
        let spec = Rc::new(SelectSpec {
            id: "excluded-apps",
            current: None,
            placeholder: "Search installed apps...",
            options: Rc::new(options),
            search: Some(SearchSpec {
                placeholder: "Search installed apps...",
                empty_message: "No apps found.",
                width: None,
                placement,
            }),
            on_select: Rc::new(|this, value, _, cx| this.toggle_ignored_app(&value, cx)),
            combobox: None,
            align_end: false,
        });
        let open = self.open_select.as_ref().filter(|open| open.id == spec.id);
        let spec_for_click = spec.clone();

        let mut trigger = div()
            .id("excluded-apps-trigger")
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .min_h(px(38.0))
            .w_full()
            .rounded(px(16.0))
            .border_1()
            .border_color(theme.border)
            .p_2()
            .cursor_text()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                if this
                    .open_select
                    .as_ref()
                    .is_some_and(|open| open.id == spec_for_click.id)
                {
                    this.close_select(cx);
                } else {
                    this.open_select(&spec_for_click, window, cx);
                }
            }));
        for bundle_id in ignored_ids {
            let is_default = defaults.contains(&bundle_id);
            let name = installed
                .iter()
                .find(|app| app.id == bundle_id)
                .map(|app| app.name.clone())
                .unwrap_or_else(|| bundle_id.clone());
            let id_for_click = bundle_id.clone();
            trigger = trigger.child(
                // `Badge variant="secondary"` in `px-2 py-0.5 text-xs`; the
                // control squircle stands in for its `rounded-full`.
                div()
                    .id(SharedString::from(format!("excluded-app-{bundle_id}")))
                    .relative()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(crate::squircle::squircle(
                        crate::squircle::CONTROL_RADIUS,
                        Some(if is_default {
                            theme.accent
                        } else {
                            theme.muted
                        }),
                        None,
                    ))
                    // `border border-transparent px-2 py-0.5`: the fill runs
                    // under the transparent border, so it is padding here.
                    .px(px(9.0))
                    .py(px(3.0))
                    .tw_text_xs()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(if is_default {
                        theme.muted_foreground
                    } else {
                        theme.foreground
                    })
                    .child(SharedString::from(name))
                    .when(is_default, |badge| {
                        badge.child(div().text_size(px(10.0)).opacity(0.7).child("(default)"))
                    })
                    .child(
                        // `Button variant="ghost" size="sm" className="ml-0.5 h-3 w-3 p-0"`
                        div()
                            .id(SharedString::from(format!(
                                "excluded-app-remove-{bundle_id}"
                            )))
                            .ml(px(2.0))
                            .size(px(12.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.toggle_ignored_app(&id_for_click, cx);
                            }))
                            .child(icon("x", px(10.0), theme.foreground)),
                    ),
            );
        }
        trigger = trigger.child(
            div()
                .tw_text_sm()
                .text_color(theme.muted_foreground)
                .child("Search installed apps..."),
        );

        let bounds_cell = self.excluded_apps_bounds.clone();
        div()
            .relative()
            .w_full()
            .on_children_prepainted(move |bounds, _, _| {
                if let Some(trigger) = bounds.first() {
                    bounds_cell.set(Some(*trigger));
                }
            })
            .child(trigger)
            .when_some(open, |wrapper, open| {
                let search = spec.search.as_ref().expect("the picker is searchable");
                let panel = self.render_searchable_panel(&spec, search, open, cx);
                wrapper.child(gpui::deferred(panel).with_priority(1))
            })
    }

    /// `buildWebAppUrl("/auth")` with the desktop flow and deep-link scheme.
    pub(super) fn auth_url(&self) -> String {
        format!(
            "{}/auth?flow=desktop&scheme={}",
            web_app_url(),
            crate::deeplink::scheme(self.store.identifier())
        )
    }

    /// `SettingsAccount` while signed out: the sign-in section with the
    /// `rounded-pill h-10 border-2 px-6` primary button that runs `signIn`
    /// (the browser hand-off behind the instruction screen).
    fn render_account_signed_out(&self, cx: &Context<Self>) -> Div {
        let theme = self.theme;
        let hovered = self.hovered == Some("account-get-started");
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
                    .on_click(
                        cx.listener(|this, _: &ClickEvent, window, cx| this.sign_in(window, cx)),
                    )
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
                    badges: Vec::new(),
                    lock: None,
                    heading: None,
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
                                        badges: Vec::new(),
                                        lock: None,
                                        heading: None,
                                    })
                                    .chain((!microphone.is_empty()).then(|| SelectOption {
                                        value: microphone.clone(),
                                        label: format!("{microphone} (Unavailable — using current default)"),
                                        detail: None,
                                        glyph: None,
                                        badges: Vec::new(),
                                        lock: None,
                                        heading: None,
                                    }))
                                    .collect(),
                                ),
                                cx,
                            ),
                        ))
                        // `AudioSettings`: Microphone, Audio file retention, Remember speakers.
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
                        )),
                ),
            )
    }

    /// `AppSettingsView` + Language & Region + Storage.
    /// Incremental-migration escape hatch: only offered when the Tauri build
    /// is installed alongside this binary, which is the packaged layout.
    fn render_classic_shell_row(&self, cx: &Context<Self>) -> Option<Div> {
        let theme = self.theme;
        crate::shell::tauri_binary(self.store.identifier())?;
        // `Button variant="outline" size="sm"`: `h-7 px-2 text-xs`.
        let button = div()
            .id("setting-classic-shell")
            .flex()
            .h(px(28.0))
            .items_center()
            .px_2()
            .rounded(px(8.0))
            .border_1()
            .border_color(theme.border)
            .bg(theme.background)
            .tw_text_xs()
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(theme.foreground)
            .cursor_pointer()
            .hover(|s| s.bg(theme.accent))
            .on_click(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                match crate::shell::switch_to_tauri(this.store.identifier()) {
                    Ok(()) => cx.quit(),
                    Err(error) => tracing::warn!(%error, "failed to switch to the classic app"),
                }
            }))
            .child("Switch back");
        Some(setting_row(
            theme,
            "Use the classic app",
            Some("Return to the previous Anarlog interface. Anarlog relaunches immediately."),
            true,
            button.into_any_element(),
        ))
    }

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
        let vault = self.store.vault_base().display().to_string();
        let home = dirs::home_dir().map(|p| p.display().to_string());
        let vault_display = crate::storage::display_path(&vault, home.as_deref());

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
                    ))
                    .children(self.render_classic_shell_row(cx)),
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
                                                    badges: Vec::new(),
                                                    lock: None,
                                                    heading: None,
                                                })
                                                .collect(),
                                        ),
                                        search: Some(SearchSpec {
                                            placeholder: "Search language...",
                                            empty_message: "No matching languages found",
                                            width: None,
                                            placement: PanelPlacement::Select,
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
                                        combobox: None,
                                        align_end: false,
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
                                                    badges: Vec::new(),
                                                    lock: None,
                                                    heading: None,
                                                })
                                                .collect(),
                                        ),
                                        search: Some(SearchSpec {
                                            placeholder: "Search timezone...",
                                            empty_message: "No results found.",
                                            width: Some(288.0),
                                            placement: PanelPlacement::Select,
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
                                        combobox: None,
                                        align_end: false,
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
                                                badges: Vec::new(),
                                                lock: None,
                                                heading: None,
                                            },
                                            SelectOption {
                                                value: "monday".to_string(),
                                                label: "Monday".to_string(),
                                                detail: None,
                                                glyph: None,
                                                badges: Vec::new(),
                                                lock: None,
                                                heading: None,
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
                div()
                    .child(section_heading(theme, "Storage"))
                    .child(self.render_storage_row(vault, vault_display, cx)),
            )
    }

    /// `StorageLocationRow`: `grid-cols-[minmax(0,1fr)_9rem] gap-3`. The row
    /// opens the folder (`openerCommands.openPath`); `Change` picks a folder,
    /// moves the vault and relaunches; an error shows under the row.
    fn render_storage_row(&self, vault: String, vault_display: String, cx: &Context<Self>) -> Div {
        let theme = self.theme;
        let pending = self.storage_change_pending;
        let open_path = vault.clone();
        div()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        // `hover:bg-muted/40 rounded-lg px-2 py-2`
                        div()
                            .id("setting-storage-open")
                            .flex()
                            .min_w_0()
                            .flex_1()
                            .items_center()
                            .gap_2()
                            .rounded_lg()
                            .px_2()
                            .py_2()
                            .cursor_pointer()
                            .hover(|s| s.bg(alpha(theme.muted, 0.4)))
                            // `openerCommands.openPath`: `xdg-open <folder>`.
                            .on_click(move |_: &gpui::ClickEvent, _, _| {
                                crate::opener::open_path(std::path::Path::new(&open_path));
                            })
                            .child(icon("folder", px(16.0), theme.muted_foreground))
                            .child(
                                // `min-w-0` alone makes Taffy size the
                                // column at its min-content width and wrap
                                // the label; `flex-1` keeps the web view's
                                // max-content layout with the path truncating.
                                div()
                                    .min_w_0()
                                    .flex_1()
                                    .child(
                                        div()
                                            .tw_text_sm()
                                            .font_weight(gpui::FontWeight::MEDIUM)
                                            .child("Where your notes and recordings are stored"),
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
                            .id("setting-storage-change")
                            .relative()
                            .w(px(144.0))
                            .h(px(36.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .gap_2()
                            .child(crate::squircle::squircle(
                                crate::squircle::CONTROL_RADIUS,
                                Some(theme.background),
                                Some((1.0, theme.border)),
                            ))
                            .tw_text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .when(pending, |button| button.opacity(0.5))
                            .when(!pending, |button| {
                                button.cursor_pointer().on_click(cx.listener(
                                    |this, _: &gpui::ClickEvent, window, cx| {
                                        this.change_storage_location(window, cx);
                                    },
                                ))
                            })
                            .when(pending, |button| {
                                button.child(crate::ui::spinner(
                                    "setting-storage-spinner",
                                    px(16.0),
                                    theme.foreground,
                                ))
                            })
                            .child("Change"),
                    ),
            )
            .when_some(self.storage_change_error.clone(), |row, error| {
                // `mt-1 text-xs text-red-500`
                row.child(
                    div()
                        .mt_1()
                        .tw_text_xs()
                        .text_color(gpui::rgb(0xef4444))
                        .child(SharedString::from(error)),
                )
            })
    }

    /// `handleChange`: a folder dialog seeded with the current location; a
    /// different choice runs `moveVault` and then relaunches the app the way
    /// `scheduleAutomaticRelaunch` does.
    fn change_storage_location(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.storage_change_pending {
            return;
        }
        let current = self.store.vault_base().to_path_buf();
        let picker = crate::dialogs::pick(
            cx,
            crate::dialogs::Options {
                title: "Choose storage location".into(),
                pick: crate::dialogs::Pick::Folder,
                start_dir: Some(current.clone()),
                filters: Vec::new(),
            },
        );
        cx.spawn_in(window, async move |this, cx| {
            let Some(paths) = picker.await else {
                return;
            };
            let Some(selected) = paths.into_iter().next() else {
                return;
            };
            if selected == current {
                return;
            }
            let task = this
                .update(cx, |this, cx| {
                    this.storage_change_pending = true;
                    this.storage_change_error = None;
                    cx.notify();
                    this.store.move_vault(selected)
                })
                .ok();
            let Some(task) = task else {
                return;
            };
            let result = task.await.map_err(anyhow::Error::from).and_then(|r| r);
            this.update(cx, |this, cx| match result {
                Ok(()) => {
                    if let Err(error) = crate::shell::relaunch_self(this.store.path()) {
                        this.storage_change_pending = false;
                        this.storage_change_error = Some(error.to_string());
                        cx.notify();
                    } else {
                        cx.quit();
                    }
                }
                Err(error) => {
                    this.storage_change_pending = false;
                    this.storage_change_error = Some(error.to_string());
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
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
pub(super) fn setting_row(
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
                    // `<p className="text-muted-foreground text-xs">`: `pretty`,
                    // on WebKit's floored 15px line.
                    column.child(div().w_full().child(crate::ui::pretty_paragraph(
                        description,
                        px(12.0),
                        px(15.0),
                        theme.muted_foreground,
                    )))
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

/// `getBaseLanguageDisplayName`: the English `Intl.DisplayNames` label of
/// the code's language, the code itself when it is not a core language.
pub(super) fn base_language_display_name(code: &str) -> String {
    let base = base_language_code(code);
    CORE_LANGUAGES
        .iter()
        .find(|(candidate, _)| *candidate == base)
        .map(|(_, label)| (*label).to_string())
        .unwrap_or_else(|| code.to_string())
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
    /// The STT model select's chips after the label (`DeprecatedBadge`,
    /// `ModelModeBadge`); `Live` shows on the selected value only, as the
    /// rows badge the batch models alone.
    pub badges: Vec<SelectBadge>,
    /// A row that cannot be picked yet: the STT page's undownloaded models
    /// read muted and offer their action on hover instead of selecting.
    pub lock: Option<SelectLock>,
    /// `getModelCategoryLabel` above the row when its category starts
    /// (`RECOMMENDED` over the Anarlog cloud model).
    pub heading: Option<&'static str>,
}

/// What an unselectable STT model row offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SelectLock {
    /// `Upgrade to use` on the Anarlog cloud model without a paid plan
    /// (`onStartTrial` → `upgradeToPro`).
    UpgradeToUse,
}

/// The chips of the STT model rows and selected value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SelectBadge {
    Deprecated,
    Live,
    AfterRecording,
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
                "End-to-end encrypted sync across 3 devices",
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
                "Sync across 5 devices per member",
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
pub(super) fn web_app_url() -> &'static str {
    if cfg!(debug_assertions) {
        "http://localhost:3000"
    } else {
        "https://anarlog.so"
    }
}

/// `getScheme()` in `shared/utils.ts`.
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
    pub placement: PanelPlacement,
}

/// Where the searchable panel sits relative to its anchor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PanelPlacement {
    /// `SelectContent position="popper"`: 40px under the 36px select trigger.
    Select,
    /// A Radix popover under a trigger of any height (`sideOffset` 4).
    Below,
    /// The popover flipped above its trigger when it would leave the window.
    Above,
}

type OnSelect = Rc<dyn Fn(&mut Workspace, String, &mut Window, &mut Context<Workspace>)>;

pub(crate) struct SelectSpec {
    pub id: &'static str,
    pub current: Option<String>,
    pub placeholder: &'static str,
    pub options: Rc<Vec<SelectOption>>,
    pub search: Option<SearchSpec>,
    pub on_select: OnSelect,
    /// `ModelCombobox` extras over the searchable panel.
    pub combobox: Option<Rc<ComboboxExtras>>,
    /// `SelectContent align="end"`: the panel grows past the trigger's width
    /// to fit its widest row and keeps its right edge on the trigger's.
    pub align_end: bool,
}

/// `ModelCombobox`: the freeform `Select "…"` row, the ignored models behind
/// the eye toggle, the loading state and the refresh button.
pub(crate) struct ComboboxExtras {
    /// `disabled || isLoadingModels`: the trigger reads `Loading models...`.
    pub loading: bool,
    /// `isConfigured`: a green check replaces the caret.
    pub configured: bool,
    /// `HealthStatusIndicator`: the probe is running.
    pub pending: bool,
    /// `getDisplayName(value)` for a selected model missing from the list.
    pub current_label: Option<String>,
    /// `(id, display name, deprecated, formatted ignore reasons)` of the
    /// ignored models.
    pub ignored: Vec<(String, String, bool, Vec<String>)>,
    /// The selected value is an ignored `old_model`.
    pub selected_deprecated: bool,
    pub on_refresh: OnRefresh,
}

type OnRefresh = Rc<dyn Fn(&mut Workspace, &mut Context<Workspace>)>;

impl SelectSpec {
    /// A select that writes its value straight to `key`.
    pub(super) fn for_setting(
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
            combobox: None,
            align_end: false,
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
    /// `showIgnored` of the model combobox.
    show_ignored: bool,
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
                        TextInputEvent::Committed
                        | TextInputEvent::BackspaceEmpty
                        | TextInputEvent::ShiftEnter
                        | TextInputEvent::ModEnter => {}
                    }
                },
            )
            .detach();
            input.read(cx).focus_handle(cx).focus(window);
            input
        });
        // Radix `Select` focuses the selected item when the list opens.
        let highlighted = match &search {
            Some(_) => 0,
            None => spec
                .current
                .as_ref()
                .and_then(|value| {
                    spec.options
                        .iter()
                        .position(|option| &option.value == value)
                })
                .unwrap_or(0),
        };
        self.open_select = Some(OpenSelect {
            id: spec.id,
            options: spec.options.clone(),
            on_select: spec.on_select.clone(),
            highlighted,
            search,
            show_ignored: false,
        });
        cx.notify();
    }

    pub(super) fn render_select(&self, spec: SelectSpec, cx: &Context<Self>) -> AnyElement {
        self.render_select_sized(spec, false, cx)
    }

    /// `compact`: the bare `SelectTrigger` with `h-8 text-xs` (`rounded-full
    /// border bg-transparent shadow-xs`) used by the automation builder.
    pub(super) fn render_select_sized(
        &self,
        spec: SelectSpec,
        compact: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        let selected = spec
            .current
            .as_ref()
            .and_then(|value| spec.options.iter().find(|option| &option.value == value));
        let combobox = spec.combobox.clone();
        let (text, color) = match selected {
            Some(option) => (
                SharedString::from(match option.detail {
                    Some(detail) => format!("{} ({detail})", option.label),
                    None => option.label.clone(),
                }),
                theme.foreground,
            ),
            // `ModelCombobox` shows the stored model's name even before (or
            // without) the catalogue listing it.
            None => match combobox
                .as_ref()
                .and_then(|extras| extras.current_label.clone())
                .filter(|_| spec.current.as_ref().is_some_and(|value| !value.is_empty()))
            {
                Some(label) => (
                    SharedString::from(label),
                    if combobox
                        .as_ref()
                        .is_some_and(|extras| extras.selected_deprecated)
                    {
                        theme.muted_foreground
                    } else {
                        theme.foreground
                    },
                ),
                None => (
                    SharedString::from(match &combobox {
                        Some(extras) if extras.loading => "Loading models...",
                        _ => spec.placeholder,
                    }),
                    theme.muted_foreground,
                ),
            },
        };
        let selected_deprecated = combobox
            .as_ref()
            .is_some_and(|extras| extras.selected_deprecated && spec.current.is_some());
        // `ModelSelectedValue`: the label at `opacity-60 text-muted-foreground`
        // for a deprecated model, then its `DeprecatedBadge` and `ModelModeBadge`.
        let selected_badges: Vec<SelectBadge> = selected
            .map(|option| option.badges.clone())
            .unwrap_or_default();
        let color = if selected_badges.contains(&SelectBadge::Deprecated) {
            theme.muted_foreground
        } else {
            color
        };
        let configured = combobox.as_ref().is_some_and(|extras| extras.configured);
        let pending = combobox.as_ref().is_some_and(|extras| extras.pending);
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
                    .h(px(if compact { 32.0 } else { 36.0 }))
                    .w_full()
                    .items_center()
                    .justify_between()
                    // `useSquircleRef` replaces the pill radius with the
                    // control squircle.
                    .when(!compact, |trigger| {
                        trigger.child(crate::squircle::squircle(
                            crate::squircle::CONTROL_RADIUS,
                            Some(theme.card),
                            Some((1.0, theme.border)),
                        ))
                    })
                    .when(compact, |trigger| {
                        // `bg-transparent` over the card; GPUI shadows need an
                        // opaque fill or they show through as a tint.
                        trigger
                            .rounded(px(8.0))
                            .border_1()
                            .border_color(theme.border)
                            .bg(theme.card)
                            .shadow_xs()
                    })
                    .px_3()
                    .py_2()
                    .when(!compact, |trigger| trigger.tw_text_sm())
                    .when(compact, |trigger| trigger.tw_text_xs())
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
                            .children(
                                selected_glyph.map(|glyph| {
                                    super::ai_settings::provider_slot_icon(glyph, theme)
                                }),
                            )
                            .child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .when(
                                        selected_badges.contains(&SelectBadge::Deprecated),
                                        |label| label.opacity(0.6),
                                    )
                                    .child(text),
                            )
                            .when(selected_deprecated, |row| row.child(deprecated_badge()))
                            .children(selected_badges.iter().map(|badge| {
                                self.select_badge(*badge, format!("select-value-{id}"), cx)
                            }))
                            // `suffix={<HealthStatusIndicator />}`
                            .when(pending, |row| {
                                row.child(div().ml_auto().child(crate::ui::spinner(
                                    SharedString::from(format!("select-health-{id}")),
                                    px(14.0),
                                    theme.muted_foreground,
                                )))
                            }),
                    )
                    .child(if configured {
                        icon("check", px(16.0), gpui::rgb(0x16a34a))
                    } else {
                        icon("caret-down", px(16.0), alpha(theme.foreground, 0.5))
                    }),
            )
            .when_some(open, |wrapper, open| {
                let panel = match &spec.search {
                    Some(search) => self.render_searchable_panel(&spec, search, open, cx),
                    None => self.render_select_panel(&spec, compact, cx),
                };
                wrapper.child(gpui::deferred(panel).with_priority(1))
            })
            .into_any_element()
    }

    /// `SelectContent position="popper"`: `bg-popover rounded-[18px] border
    /// shadow-md p-1` at the trigger's width, 4px below; items `py-1.5 pr-8
    /// pl-2 text-sm` with the check at `right-2`.
    fn render_select_panel(
        &self,
        spec: &SelectSpec,
        compact: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        let id = spec.id;
        let highlighted = self
            .open_select
            .as_ref()
            .filter(|open| open.id == id)
            .map(|open| open.highlighted);
        div()
            .id(SharedString::from(format!("select-content-{id}")))
            .occlude()
            .absolute()
            .top(px(if compact { 36.0 } else { 40.0 }))
            .when(!spec.align_end, |panel| {
                panel.left_0().w_full().min_w(px(128.0))
            })
            // `align="end"`: at least the trigger's width, wider for the rows.
            .when(spec.align_end, |panel| panel.right_0().min_w_full())
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
                let lock = option.lock;
                // `text-muted-foreground px-2 pt-2 pb-1 text-[11px] font-medium
                // tracking-wide uppercase` over the category's first row.
                let heading = option.heading.map(|heading| {
                    div()
                        .px_2()
                        .pt_2()
                        .pb_1()
                        .text_size(px(11.0))
                        .line_height(px(15.0))
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(theme.muted_foreground)
                        .child(SharedString::from(heading.to_uppercase()))
                });
                let row = div()
                    .id(SharedString::from(format!("select-option-{id}-{index}")))
                    .group(format!("select-option-{id}-{index}"))
                    .relative()
                    .flex()
                    // A content-sized panel stretches its rows to the widest one;
                    // a percentage width would size each to its own content.
                    .when(!spec.align_end, |row| row.w_full())
                    .items_center()
                    .py(px(6.0))
                    // The locked row keeps `pr-1.5` for its action button.
                    .when(lock.is_none(), |row| row.pr_8())
                    .when(lock.is_some(), |row| row.pr(px(6.0)))
                    .pl_2()
                    .rounded(px(14.0))
                    .tw_text_sm()
                    .text_color(if lock.is_some() {
                        theme.muted_foreground
                    } else {
                        theme.foreground
                    })
                    .cursor_default()
                    .when(lock.is_some(), |row| row.cursor_pointer())
                    // `focus:bg-accent`: the focused item, which the pointer moves.
                    .when(highlighted == Some(index), |row| row.bg(theme.accent))
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
                        // `handleAction`: the whole row runs the action.
                        if let Some(lock) = lock {
                            this.close_select(cx);
                            this.run_select_lock(lock, window, cx);
                            return;
                        }
                        this.close_select(cx);
                        on_select(this, value.clone(), window, cx);
                    }))
                    .when_some(lock, |row, lock| {
                        // `Upgrade to use`: `bg-primary text-primary-foreground rounded-full
                        // px-2 py-1 text-[11px] font-medium shadow-xs`, shown on hover.
                        row.child(
                            div()
                                .absolute()
                                .right(px(6.0))
                                .top_0()
                                .bottom_0()
                                .flex()
                                .items_center()
                                .child(
                                    div()
                                        .invisible()
                                        .group_hover(
                                            format!("select-option-{id}-{index}"),
                                            |button| button.visible(),
                                        )
                                        .rounded(px(9999.0))
                                        .px_2()
                                        .py(px(4.0))
                                        .text_size(px(11.0))
                                        .line_height(px(16.0))
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .bg(theme.primary)
                                        .text_color(theme.primary_foreground)
                                        .shadow_xs()
                                        .child(match lock {
                                            SelectLock::UpgradeToUse => "Upgrade to use",
                                        }),
                                ),
                        )
                    })
                    .when_some(option.glyph, |row, glyph| {
                        row.gap_2()
                            .child(super::ai_settings::provider_slot_icon(glyph, theme))
                    })
                    // A deprecated model's row reads `text-muted-foreground`.
                    .when(option.badges.contains(&SelectBadge::Deprecated), |row| {
                        row.text_color(theme.muted_foreground)
                    })
                    .child(if option.badges.is_empty() {
                        SharedString::from(option.label.clone()).into_any_element()
                    } else {
                        // `SelectItemText`'s inline span shrinks the `justify-between`
                        // content to the label plus its `gap-3` chip group; the
                        // rows badge deprecation and the batch mode only.
                        div()
                            .flex()
                            .min_w_0()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .whitespace_nowrap()
                                    .child(SharedString::from(option.label.clone())),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_shrink_0()
                                    .items_center()
                                    .gap_2()
                                    .children(
                                        option
                                            .badges
                                            .iter()
                                            .filter(|badge| **badge != SelectBadge::Live)
                                            .map(|badge| {
                                                self.select_badge(
                                                    *badge,
                                                    format!("select-option-{id}-{index}"),
                                                    cx,
                                                )
                                            }),
                                    ),
                            )
                            .into_any_element()
                    })
                    .when(selected, |item| {
                        item.child(div().absolute().right_2().flex().items_center().child(icon(
                            "check",
                            px(16.0),
                            theme.foreground,
                        )))
                    });
                match heading {
                    Some(heading) => div()
                        .flex()
                        .flex_col()
                        .child(heading)
                        .child(row)
                        .into_any_element(),
                    None => row.into_any_element(),
                }
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
        let combobox = spec.combobox.clone();
        let trimmed_query = query.trim().to_string();
        // `canSelectFreeform`: a typed id that matches no option exactly.
        let freeform = combobox.is_some()
            && !trimmed_query.is_empty()
            && !spec
                .options
                .iter()
                .any(|option| option.value.to_lowercase() == trimmed_query.to_lowercase());
        let ignored_rows: Vec<(String, String, bool, Vec<String>)> = combobox
            .as_ref()
            .filter(|_| open.show_ignored)
            .map(|extras| {
                let lower = query.to_lowercase();
                extras
                    .ignored
                    .iter()
                    .filter(|(id, label, _, _)| {
                        lower.is_empty() || format!("{id} {label}").to_lowercase().contains(&lower)
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        let empty_message: SharedString = match &combobox {
            Some(extras) => SharedString::from(if !trimmed_query.is_empty() {
                "No results found."
            } else if !extras.ignored.is_empty() {
                "No models ready to use."
            } else {
                "No models available."
            }),
            None => SharedString::from(search.empty_message),
        };

        let mut panel = div()
            .id(SharedString::from(format!("select-content-{id}")))
            .occlude()
            .absolute()
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
        panel = match search.placement {
            PanelPlacement::Select => panel.top(px(40.0)),
            PanelPlacement::Below => panel.top(relative(1.0)).mt(px(4.0)),
            PanelPlacement::Above => panel.bottom(relative(1.0)).mb(px(4.0)),
        };

        let mut list = div()
            .id(SharedString::from(format!("select-list-{id}")))
            .flex()
            .flex_col()
            .max_h(px(250.0))
            .overflow_y_scroll()
            .p_1();
        if matches.is_empty() && ignored_rows.is_empty() && !freeform {
            list = list.child(
                div()
                    .px_2()
                    .py(px(6.0))
                    .tw_text_sm()
                    .text_color(theme.muted_foreground)
                    .child(empty_message),
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
                    .when(option.badges.contains(&SelectBadge::Deprecated), |row| {
                        row.text_color(theme.muted_foreground)
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(SharedString::from(option.label.clone())),
                    )
                    // The rows badge deprecation and the batch mode only.
                    .children(
                        option
                            .badges
                            .iter()
                            .filter(|badge| **badge != SelectBadge::Live)
                            .map(|badge| {
                                self.select_badge(*badge, format!("select-row-{id}-{index}"), cx)
                            }),
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
                    // cmdk items carry no check indicator in the combobox.
                    .when(selected && combobox.is_none(), |row| {
                        row.child(icon("check", px(16.0), theme.foreground))
                    })
            }));
            // `showIgnored`: the filtered-out models at half opacity, the
            // deprecated ones badged.
            list = list.children(ignored_rows.into_iter().enumerate().map(
                |(index, (value, label, deprecated, reasons))| {
                    let on_select = spec.on_select.clone();
                    // `Tooltip delayDuration={10}` / `side="right"`: one
                    // `• reason` line per `formatIgnoreReason`.
                    let tooltip = super::tooltip::TooltipSpec::lines(
                        format!("select-ignored-{id}-{index}"),
                        reasons.iter().map(|reason| format!("• {reason}")).collect(),
                        super::tooltip::Side::Right,
                    )
                    .delay(10);
                    div()
                        .id(SharedString::from(format!("select-ignored-{id}-{index}")))
                        .flex()
                        .w_full()
                        .items_center()
                        .gap_2()
                        .px_2()
                        .py(px(6.0))
                        .rounded(px(14.0))
                        .tw_text_sm()
                        .text_color(theme.foreground)
                        .opacity(0.5)
                        .cursor_pointer()
                        .hover(move |style| style.bg(theme.accent))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            this.close_select(cx);
                            this.focus_handle.focus(window);
                            on_select(this, value.clone(), window, cx);
                        }))
                        .child(
                            self.tooltip_trigger(
                                tooltip,
                                div()
                                    .flex()
                                    .w_full()
                                    .min_w_0()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div().min_w_0().truncate().child(SharedString::from(label)),
                                    )
                                    .when(deprecated, |row| row.child(deprecated_badge())),
                                cx,
                            ),
                        )
                },
            ));
            // `Select "query"`
            if freeform {
                let value = trimmed_query.clone();
                let on_select = spec.on_select.clone();
                list = list.child(
                    div()
                        .id(SharedString::from(format!("select-freeform-{id}")))
                        .flex()
                        .w_full()
                        .items_center()
                        .px_2()
                        .py(px(6.0))
                        .rounded(px(14.0))
                        .tw_text_sm()
                        .text_color(theme.foreground)
                        .cursor_pointer()
                        .hover(move |style| style.bg(theme.accent))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            this.close_select(cx);
                            this.focus_handle.focus(window);
                            on_select(this, value.clone(), window, cx);
                        }))
                        .child(icon("plus-circle", px(16.0), theme.foreground))
                        .child(div().w(px(8.0)))
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .child(SharedString::from(format!("Select \"{trimmed_query}\""))),
                        ),
                );
            }
        }
        // `border-t px-2 py-1.5 text-xs`: the eye toggle, the count and the
        // refresh button.
        let footer = combobox.as_ref().map(|extras| {
            let show_ignored = open.show_ignored;
            let ignored_count = extras.ignored.len();
            let shown = spec.options.len();
            let on_refresh = extras.on_refresh.clone();
            let loading = extras.loading;
            div()
                .flex()
                .items_center()
                .justify_between()
                .border_t_1()
                .border_color(theme.border)
                .px_2()
                .py(px(6.0))
                .tw_text_xs()
                .text_color(theme.muted_foreground)
                .child(
                    div()
                        .id(SharedString::from(format!("select-ignored-toggle-{id}")))
                        .flex()
                        .items_center()
                        .gap_1()
                        .mr_1()
                        .cursor_pointer()
                        .hover(move |style| style.text_color(theme.foreground))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            if let Some(open) = this.open_select.as_mut() {
                                open.show_ignored = !open.show_ignored;
                                cx.notify();
                            }
                        }))
                        .child(icon(
                            if show_ignored { "eye-slash" } else { "eye" },
                            px(12.0),
                            theme.muted_foreground,
                        )),
                )
                .when(ignored_count > 0, |footer| {
                    footer.child(SharedString::from(if show_ignored {
                        format!("Showing total of {shown} models.")
                    } else {
                        format!("{ignored_count} items ignored.")
                    }))
                })
                .child(
                    div()
                        .id(SharedString::from(format!("select-refresh-{id}")))
                        .ml_auto()
                        .flex()
                        .items_center()
                        .gap_1()
                        .when(loading, |button| button.opacity(0.5))
                        .when(!loading, |button| {
                            button
                                .cursor_pointer()
                                .hover(move |style| style.text_color(theme.foreground))
                        })
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            if !loading {
                                on_refresh(this, cx);
                            }
                        }))
                        .child(icon(
                            "arrows-counter-clockwise",
                            px(12.0),
                            theme.muted_foreground,
                        )),
                )
        });

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
                    .child(list)
                    .children(footer),
            )
            .into_any_element()
    }
}

impl Workspace {
    /// The action behind a locked select row.
    fn run_select_lock(&mut self, lock: SelectLock, window: &mut Window, cx: &mut Context<Self>) {
        match lock {
            SelectLock::UpgradeToUse => self.upgrade_to_pro(window, cx),
        }
    }

    /// A `SelectBadge`: the `DeprecatedBadge`, or `ModelModeBadge`'s `Live`
    /// (`bg-sky-50 text-sky-700`) / `After recording` (`bg-muted
    /// text-muted-foreground`) chip with its `delayDuration={100}` tooltip.
    fn select_badge(&self, badge: SelectBadge, id: String, cx: &Context<Self>) -> AnyElement {
        let theme = self.theme;
        match badge {
            SelectBadge::Deprecated => deprecated_badge().into_any_element(),
            SelectBadge::Live | SelectBadge::AfterRecording => {
                let live = badge == SelectBadge::Live;
                let chip = div()
                    .flex_shrink_0()
                    .rounded(px(6.0))
                    .px(px(6.0))
                    .py(px(2.0))
                    .text_size(px(11.0))
                    .line_height(px(16.0))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .when(live, |chip| {
                        chip.bg(gpui::rgb(0xf0f9ff)).text_color(gpui::rgb(0x0369a1))
                    })
                    .when(!live, |chip| {
                        chip.bg(theme.muted).text_color(theme.muted_foreground)
                    })
                    .child(if live { "Live" } else { "After recording" });
                self.tooltip_trigger(
                    super::tooltip::TooltipSpec::text(
                        id,
                        if live {
                            "Can transcribe while the meeting is happening."
                        } else {
                            "Runs after the recording finishes, not during the meeting."
                        },
                        super::tooltip::Side::Top,
                    )
                    .max_width(256.0)
                    .delay(100),
                    chip,
                    cx,
                )
                .into_any_element()
            }
        }
    }
}

/// `DeprecatedBadge`: `rounded-md px-1.5 py-0.5 text-[11px] font-medium
/// bg-amber-50 text-amber-800`.
fn deprecated_badge() -> gpui::Div {
    div()
        .flex_shrink_0()
        .rounded(px(6.0))
        .px(px(6.0))
        .py(px(2.0))
        .text_size(px(11.0))
        .line_height(px(16.0))
        .font_weight(gpui::FontWeight::MEDIUM)
        .bg(gpui::rgb(0xfffbeb))
        .text_color(gpui::rgb(0x92400e))
        .child("Deprecated")
}
